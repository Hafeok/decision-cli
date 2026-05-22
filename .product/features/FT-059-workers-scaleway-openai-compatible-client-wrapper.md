---
id: FT-059
title: 'workers: Scaleway OpenAI-compatible client wrapper in _shared'
phase: 2
status: planned
depends-on: []
adrs:
- ADR-037
- ADR-008
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

## Functional Specification

### Inputs

- The `openai` Python SDK (`pip install openai`) added as a dependency of `workers/_shared`.
- `SCW_SECRET_KEY` environment variable available to the worker process.
- A capability-resolved dispatch payload from [FT-061](FT-061): `{ endpoint: "scaleway", model_identifier: "<exact-id>", parameters: {…} }`.

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
- A `ModelCaller`-shaped helper compatible with the verifier worker's existing `ModelCaller` signature (`(system, user, model_id, max_tokens) -> (raw_json_string, tokens_in, tokens_out)`):
  ```python
  def scaleway_chat_caller(client: OpenAI) -> ModelCaller: ...
  # returns a callable that issues a chat.completions.create call and
  # returns the text + usage tuple in the same shape as verifier's call_claude.
  ```
- Optional tool-call and structured-output adapters used by [FT-060](FT-060):
  - `def scaleway_tool_caller(client, tools, tool_choice="auto")` — supports OpenAI function-tools format.
  - `def scaleway_json_caller(client, response_schema)` — uses `response_format = {"type": "json_schema", ...}` with the SHACL-derived JSON schema.

### State

- The client is constructed per worker process and reused (the `openai.OpenAI` client manages its own HTTP connection pool). The wrapper does *not* maintain global state — every call creates a fresh client unless the caller caches one.

### Behaviour

1. `build_client` returns `OpenAI(base_url=SCALEWAY_BASE_URL, api_key=environ[SCALEWAY_KEY_ENV])`. Raises `ScalewayClientError` with a specific message if `SCW_SECRET_KEY` is missing.
2. `missing_key_error_or_none` returns an error object (not raising) so callers can surface "the worker cannot dispatch because Scaleway key is missing" as a session error vs. crashing.
3. `scaleway_chat_caller` returns a closure that:
   - Issues `client.chat.completions.create(model=model_id, messages=[{role:"system",…},{role:"user",…}], max_tokens=max_tokens, temperature=<from params or 0.0>)`.
   - Extracts `response.choices[0].message.content` as the text.
   - Extracts `response.usage.prompt_tokens` and `response.usage.completion_tokens` as `(tokens_in, tokens_out)`.
   - Returns `(text, tokens_in, tokens_out)`.
4. `scaleway_tool_caller` passes `tools=tools` and `tool_choice=tool_choice` through to the API; returns the structured tool-call response shape that [FT-060](FT-060)'s adapter consumes.
5. `scaleway_json_caller` passes `response_format={"type": "json_schema", "json_schema": response_schema}`; the open question from PRD §14 (whether Scaleway requires an `extra_body` field for non-standard parameters like `reasoning_effort`) is verified empirically and the wrapper documents whichever path Scaleway accepts.
6. All callers honour `parameters.reasoning_effort` from the dispatch payload when present; on `gpt-oss-120b` and any other `configurable_effort = true` capability ([FT-054](FT-054)), the value is passed through. See [FT-063](FT-063) for the stakes→effort mapping.

### Invariants

- The wrapper does not read or write the orchestration graph (worker contract per [ADR-008](ADR-008)).
- The wrapper does not catch and retry network errors silently — failures surface as `ScalewayClientError` so the dispatcher can record the session as failed and (optionally) escalate per [ADR-034](ADR-034).
- `SCW_SECRET_KEY` is read once per client construction; the wrapper does not log the key.
- Token counts in the returned tuple are non-negative integers; absent usage data (network error mid-response) returns `(0, 0)` with the exception still raised.

### Error handling

- `SCW_SECRET_KEY` missing → `ScalewayClientError("SCW_SECRET_KEY not set; install Scaleway key or rebind capability to endpoint=anthropic")`.
- Network error (timeout, connection refused) → `ScalewayClientError` wrapping the underlying `openai.APIError`.
- Rate limit (429) → `ScalewayClientError` with category `rate_limited`; dispatcher decides retry / fail (out of scope for this feature — PRD §13 defers rate-limit handling to the worker layer).
- Authentication failure (401) → `ScalewayClientError` with category `auth_failed`; surfaced to operator via session telemetry.
- Empty response content → returned as `("", tokens_in, tokens_out)`; downstream validation (Pydantic schema, [FT-060](FT-060)) treats empty as a validation error.

### Boundaries

- **In scope.** The client wrapper, chat-completion caller, tool-call adapter, JSON-schema adapter, env-var handling.
- **Out of scope.** Worker-specific integration (which worker calls which caller — [FT-060](FT-060)).
- **Out of scope.** Anthropic client — already present at `workers/verifier/.../worker.py`; [FT-060](FT-060) abstracts the two endpoints behind a uniform shape.
- **Out of scope.** Cross-region failover (PRD §3 deferred).
- **Out of scope.** Caching of repeated bundles (PRD §3 deferred).

## Out of scope

- A Rust wrapper for the Scaleway endpoint. Workers are Python ([ADR-008](ADR-008)); the harness does not call Scaleway directly.
- Streaming response support (the current verifier worker uses single-shot completion; streaming is a Phase 3+ concern).
- Image / vision endpoint support for the `vision-*` capabilities — those capabilities are catalog entries with no current role binding ([ADR-037](ADR-037)), so no worker integration is required yet.
- Audio transcription endpoint for `audio-transcribe` — same reasoning.
