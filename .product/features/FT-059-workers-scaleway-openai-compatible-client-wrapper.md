---
id: FT-059
title: 'workers: Scaleway OpenAI-compatible client wrapper in _shared'
phase: 2
status: planned
depends-on: []
adrs:
- ADR-001
- ADR-002
- ADR-004
- ADR-005
- ADR-008
- ADR-012
- ADR-013
- ADR-014
- ADR-015
- ADR-016
- ADR-017
- ADR-018
- ADR-020
- ADR-021
- ADR-022
- ADR-023
- ADR-024
- ADR-025
- ADR-027
- ADR-033
- ADR-034
- ADR-035
- ADR-036
- ADR-037
tests:

- TC-105
domains:
- error-handling
- networking
- security
domains-acknowledged: {}
---

## Description

Add a Scaleway OpenAI-compatible client wrapper in `workers/_shared/` so every Python worker (`code-writer`, `verifier`, future workers) can route to Scaleway's serverless inference endpoint without each worker re-implementing the SDK plumbing. The wrapper uses the official `openai` SDK against `https://api.scaleway.ai/v1`. Authentication is via `SCW_SECRET_KEY` from the worker process environment, stored alongside `ANTHROPIC_API_KEY` per [ADR-037](ADR-037).

This is the worker-side counterpart to [FT-061](FT-061)'s dispatcher capability resolution: when the dispatcher resolves a role to a capability with `endpoint = scaleway`, the worker uses this client to make the model call.

PRD §14 confirmed two Scaleway-specific behaviors this feature relies on (verified against the live API):

- **`reasoning_effort` is a standard top-level kwarg** on the OpenAI chat-completions surface for `gpt-oss-120b`. No `extra_body` workaround needed. Valid values: `'none'`, `'low'`, `'medium'`, `'high'`. Server-validated via Pydantic.
- **`message.reasoning` exposes the reasoning trace** for `qwen3.5-397b-a17b` as a top-level field on the message object (not `reasoning_content`, not `<think>` tags). Worker reads it separately from `message.content`; see [FT-060](FT-060) §10.6 for the artifact-side wiring.

## Functional Specification

### Inputs

- The `openai` Python SDK (`pip install openai`) added as a dependency of `workers/_shared`.
- `SCW_SECRET_KEY` environment variable available to the worker process.
- A capability-resolved dispatch payload from [FT-061](FT-061): `{ endpoint: "scaleway", model_identifier: "<exact-id>", parameters: {…} }` where `parameters` may include `reasoning_effort` for `configurable_effort` capabilities ([FT-063](FT-063)).

### Outputs

- New module `workers/_shared/src/_shared/scaleway_client.py` exposing:
  ```python
  from openai import OpenAI
  from os import environ
  
  SCALEWAY_BASE_URL = "https://api.scaleway.ai/v1"
  SCALEWAY_KEY_ENV = "SCW_SECRET_KEY"
  
  class ScalewayClientError(Exception): ...
  
  def build_client() -> OpenAI: ...
  def missing_key_error_or_none() -> ScalewayClientError | None: ...
  ```
- Callers compatible with the verifier worker's `ModelCaller` signature (`(system, user, model_id, max_tokens) -> (raw_json_string, tokens_in, tokens_out)`):
  ```python
  def scaleway_chat_caller(client: OpenAI, *, reasoning_effort: str | None = None) -> ModelCaller: ...
  # Returns a callable that issues chat.completions.create. When reasoning_effort
  # is set, passes it as a top-level kwarg (not extra_body). Parses message.content
  # for the text and message.reasoning for the trace; returns both via the structured
  # result shape consumed by FT-060's ModelRouter.
  ```
- Optional tool-call and structured-output adapters used by [FT-060](FT-060):
  - `def scaleway_tool_caller(client, tools, tool_choice="auto", reasoning_effort=None)` — supports OpenAI function-tools format.
  - `def scaleway_json_caller(client, response_schema, reasoning_effort=None)` — uses `response_format = {"type": "json_schema", ...}` with the SHACL-derived JSON schema.

### State

- The client is constructed per worker process and reused (the `openai.OpenAI` client manages its own HTTP connection pool). The wrapper does *not* maintain global state — every call creates a fresh client unless the caller caches one.

### Behaviour

1. `build_client` returns `OpenAI(base_url=SCALEWAY_BASE_URL, api_key=environ[SCALEWAY_KEY_ENV])`. Raises `ScalewayClientError` with a specific message if `SCW_SECRET_KEY` is missing.
2. `missing_key_error_or_none` returns an error object (not raising) so callers can surface "the worker cannot dispatch because Scaleway key is missing" as a session error vs. crashing.
3. `scaleway_chat_caller` returns a closure that:
   - Issues `client.chat.completions.create(model=model_id, messages=[{role:"system",…},{role:"user",…}], max_tokens=max_tokens, temperature=<from params or 0.0>, reasoning_effort=<from params, if set>)`. `reasoning_effort` is a top-level kwarg per PRD §14 resolution; do not wrap in `extra_body`.
   - Extracts `response.choices[0].message.content` as the text. May be `None` for capabilities with `exposes_reasoning_trace = true` mid-stream while the model is still reasoning — caller treats that as "still thinking, keep accumulating," not failure (see [FT-060](FT-060) §10.6).
   - Extracts `response.choices[0].message.reasoning` (via `getattr(message, "reasoning", None)`) when the dispatched capability has `exposes_reasoning_trace = true`. Returns this alongside the text content.
   - Extracts `response.usage.prompt_tokens` and `response.usage.completion_tokens` as `(tokens_in, tokens_out)`.
   - For Scaleway, `input_tokens_cache_write` and `input_tokens_cache_hit` are always 0 (no prompt caching on Scaleway currently). The result returns the base counts; [FT-057](FT-057)'s session record records zeros for the cache fields on Scaleway dispatches.
4. `scaleway_tool_caller` passes `tools=tools` and `tool_choice=tool_choice` through to the API; returns the structured tool-call response shape that [FT-060](FT-060)'s adapter consumes. Honours `reasoning_effort` when set.
5. `scaleway_json_caller` passes `response_format={"type": "json_schema", "json_schema": response_schema}` and honours `reasoning_effort`.
6. All callers honour `parameters.reasoning_effort` from the dispatch payload when present; on `gpt-oss-120b` and any other `configurable_effort = true` capability ([FT-054](FT-054)), the value is passed through as a top-level kwarg. Valid values: `'none'`, `'low'`, `'medium'`, `'high'`. See [FT-063](FT-063) for the stakes→effort mapping.

### Invariants

- The wrapper does not read or write the orchestration graph (worker contract per [ADR-008](ADR-008)).
- The wrapper does not catch and retry network errors silently — failures surface as `ScalewayClientError` so the dispatcher can record the session as failed and (optionally) escalate per [ADR-034](ADR-034).
- `SCW_SECRET_KEY` is read once per client construction; the wrapper does not log the key.
- Token counts in the returned tuple are non-negative integers; absent usage data (network error mid-response) returns `(0, 0)` with the exception still raised.
- `reasoning_effort` is passed as a top-level kwarg (`openai.OpenAI().chat.completions.create(..., reasoning_effort=...)`); never inside `extra_body`. Verified against the live Scaleway API per PRD §14.
- `reasoning_effort` is silently ignored by the Scaleway server for capabilities that don't support it; the wrapper does not pre-filter, but [FT-061](FT-061)'s `compute_params` only sets it when the resolved capability has `configurable_effort = true`.
- `message.reasoning` is read via `getattr` with default `None`; capabilities that don't expose it simply produce no trace, and the worker treats `None` as "no trace available".

### Error handling

- `SCW_SECRET_KEY` missing → `ScalewayClientError("SCW_SECRET_KEY not set; install Scaleway key or rebind capability to endpoint=anthropic")`.
- Network error (timeout, connection refused) → `ScalewayClientError` wrapping the underlying `openai.APIError`.
- Rate limit (429) → `ScalewayClientError` with category `rate_limited`; dispatcher decides retry / fail (out of scope for this feature — PRD §13 defers rate-limit handling to the worker layer).
- Authentication failure (401) → `ScalewayClientError` with category `auth_failed`; surfaced to operator via session telemetry.
- Empty response content with non-empty `message.reasoning` → return `("", trace, tokens_in, tokens_out)`; this is the long-chain case described in PRD §10.6 where the model is still reasoning. [FT-060](FT-060) decides whether to treat as completion or as mid-stream depending on whether the caller is streaming.
- Empty response content with empty `message.reasoning` → return `("", None, tokens_in, tokens_out)`; downstream validation treats empty content as a validation error.
- Invalid `reasoning_effort` value (e.g. typo in the dispatcher params) → Scaleway server returns a 400 with a Pydantic error message; the wrapper surfaces this as `ScalewayClientError(category="invalid_params")`.

### Boundaries

- **In scope.** The client wrapper, chat-completion caller, tool-call adapter, JSON-schema adapter, env-var handling, top-level `reasoning_effort` plumbing, `message.reasoning` extraction.
- **Out of scope.** Worker-specific integration (which worker calls which caller — [FT-060](FT-060)).
- **Out of scope.** Anthropic client — already present at `workers/verifier/.../worker.py`; [FT-060](FT-060) abstracts the two endpoints behind a uniform shape.
- **Out of scope.** Cross-region failover (PRD §3 deferred).
- **Out of scope.** Caching of repeated bundles (PRD §3 deferred; Scaleway does not currently support prompt caching).
- **Out of scope.** Attaching `message.reasoning` to the produced artifact as `rationale_trace` — that's [FT-060](FT-060)'s `ModelRouter` and the artifact schema, not the Scaleway-client wrapper.

## Out of scope

- A Rust wrapper for the Scaleway endpoint. Workers are Python ([ADR-008](ADR-008)); the harness does not call Scaleway directly.
- Streaming response support (the current verifier worker uses single-shot completion; streaming is a Phase 3+ concern).
- Image / vision endpoint support for the `vision-*` capabilities — those capabilities are catalog entries with no current role binding ([ADR-037](ADR-037)), so no worker integration is required yet.
- Audio transcription endpoint for `audio-transcribe` — same reasoning.
- Prompt caching for Scaleway — Scaleway does not currently support prompt caching; if added in future, [FT-065](FT-065)'s breakpoint logic generalises to any endpoint with non-null `cost_cache_hit_per_m`.
