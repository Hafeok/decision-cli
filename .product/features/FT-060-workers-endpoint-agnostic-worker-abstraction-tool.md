---
id: FT-060
title: 'workers: Endpoint-agnostic worker abstraction (tool schemas and structured output across Scaleway and Anthropic)'
phase: 2
status: complete
depends-on:
- FT-059
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
- TC-106
domains:
- api
- error-handling
domains-acknowledged: {}
---

## Description

Replace the worker-layer endpoint-coupling with a uniform `ModelCaller` abstraction that routes to either the Anthropic SDK or the Scaleway OpenAI-compatible client from [FT-059](FT-059), based on the resolved capability in the dispatch payload. The abstraction lives in `workers/_shared/` so every worker reuses the same shape. Tool schemas are declared once in OpenAI function-tools format and translated to Anthropic's tool-use format at call time; structured outputs use a JSON schema derived from the worker's Pydantic output model.

Per PRD §10.6, the router also handles **reasoning-trace ingestion** for capabilities that expose one: the trace is parsed alongside the message content and attached to the produced artifact as `rationale_trace` (e.g., on an ADR). Currently only `standard-reasoning-frontier` (qwen3.5-397b-a17b) emits a trace via `response.choices[0].message.reasoning`.

This is the worker-side seam that closes the loop: dispatcher resolves capability → injects `(endpoint, model, params)` into dispatch payload → worker `ModelRouter` dispatches to the right client, returning text + tools + tokens + optional reasoning trace. Workers stay stateless and free of endpoint policy ([ADR-008](ADR-008), [ADR-033](ADR-033)).

## Functional Specification

### Inputs

- The existing `ModelCaller` callable in `workers/verifier/src/verifier/worker.py:52` (signature: `(system, user, model_id, max_tokens) -> (raw_json_string, tokens_in, tokens_out)`).
- The Scaleway client wrapper from [FT-059](FT-059), including the `reasoning_effort` top-level kwarg and `message.reasoning` extraction.
- The Anthropic SDK already wired in the verifier (`call_claude` at `worker.py:66`).
- The dispatch payload from [FT-061](FT-061), which now carries `endpoint`, `model_identifier`, and `parameters` (incl. optional `reasoning_effort`, `temperature`, `tool_definitions`, `response_schema`).
- The capability's `exposes_reasoning_trace` flag from [FT-054](FT-054), available to the worker via the dispatch payload (the dispatcher forwards it as `parameters.exposes_reasoning_trace`).
- The PRD's tool list for the implementer (`read_file`, `edit_file`, `run_bash`, `record_emergent_judgment`, `file_feedback`, `submit`) authored once in OpenAI function-tools format.

### Outputs

- New module `workers/_shared/src/_shared/model_router.py`:
  ```python
  from dataclasses import dataclass
  from typing import Literal, Protocol
  
  Endpoint = Literal["scaleway", "anthropic"]
  ReasoningEffort = Literal["none", "low", "medium", "high"]
  
  @dataclass
  class CallParams:
      endpoint: Endpoint
      model_identifier: str
      max_tokens: int
      temperature: float = 0.0
      reasoning_effort: ReasoningEffort | None = None
      tools: list[dict] | None = None         # OpenAI function-tools format (canonical)
      response_schema: dict | None = None     # JSON schema for structured output
      exposes_reasoning_trace: bool = False   # parse message.reasoning when true
  
  class ModelRouter(Protocol):
      def call(self, system: str, user: str, params: CallParams) -> "ModelResponse": ...
  
  @dataclass
  class ModelResponse:
      text: str
      tool_calls: list["ToolCall"]
      tokens_in: int
      tokens_out: int
      tokens_cache_write: int                 # Anthropic only; 0 elsewhere
      tokens_cache_hit: int                   # Anthropic only; 0 elsewhere
      stop_reason: str
      rationale_trace: str | None             # populated when params.exposes_reasoning_trace and trace was emitted
  
  def build_router(endpoint: Endpoint) -> ModelRouter: ...
  ```
- Anthropic implementation: translates OpenAI tool format → Anthropic `tool_use` format at call time; reads `response.content` for text and `tool_use` blocks for tool calls; reads `response.usage.cache_creation_input_tokens` and `response.usage.cache_read_input_tokens` for the cache breakdown that [FT-057](FT-057) records on the session.
- Scaleway implementation: uses [FT-059](FT-059)'s callers; passes OpenAI tools directly; honours `reasoning_effort` for `configurable_effort` capabilities as a top-level kwarg; when `params.exposes_reasoning_trace = true`, parses `response.choices[0].message.reasoning` and surfaces as `ModelResponse.rationale_trace`.
- Tool schema module `workers/_shared/src/_shared/tools.py` with the canonical implementer tool list as OpenAI function-tools JSON. Anthropic translator lives here.
- Verifier worker (`workers/verifier/.../worker.py`) refactored to use `ModelRouter` instead of the hardcoded `call_claude`; `DEFAULT_MODEL_ID = "claude-sonnet-4-5"` removed (model id comes from dispatch payload per [FT-061](FT-061)).

### Tool schema translation

The canonical format is OpenAI function-tools. Anthropic translation is mechanical:

```python
def openai_tool_to_anthropic(tool: dict) -> dict:
    return {
        "name": tool["function"]["name"],
        "description": tool["function"]["description"],
        "input_schema": tool["function"]["parameters"],
    }
```

The reverse is needed only when the dispatcher injects an Anthropic-native tool list; the dispatcher always uses canonical OpenAI format, so the reverse direction is unused and not implemented.

### Structured output

When `params.response_schema` is set:

- **Scaleway**: pass `response_format={"type": "json_schema", "json_schema": {"name": "Verdict", "schema": params.response_schema, "strict": True}}` to `chat.completions.create`.
- **Anthropic**: emulate via the existing tool-use approach the verifier uses today — declare a single tool named `submit_verdict` whose `input_schema` is the JSON schema; force `tool_choice = {"type": "tool", "name": "submit_verdict"}`; extract the tool input as the structured output.

The verifier worker's existing single-tool pattern provides the Anthropic template; this feature generalises it.

### Reasoning-trace ingestion (PRD §10.6)

For capabilities with `exposes_reasoning_trace = true`, the router parses an additional API response field and surfaces it as `ModelResponse.rationale_trace`:

- **Scaleway / `qwen3.5-397b-a17b`**: read `response.choices[0].message.reasoning` via `getattr(msg, "reasoning", None)`. The field is *literally* `reasoning` (not `reasoning_content`, not embedded `<think>` tags). Reasoning is emitted by default (no opt-in flag needed).
- **Anthropic**: the equivalent for extended thinking lives in `response.content` blocks of type `thinking`. Not currently used — all Anthropic capabilities in the seed catalog ship with `exposes_reasoning_trace = false`. The hook is in place; populating it is a follow-up decision once Anthropic extended-thinking pricing is in the catalog.

**Long-chain handling.** Per PRD §10.6, the model may emit `message.content == None` mid-stream while reasoning is in progress, with `message.reasoning` carrying the chain. For the current single-shot synchronous calls, this means waiting for the full response — when the API returns, `content` is populated and reasoning is complete. If streaming is added later, the router accumulates both fields until completion and only then constructs `ModelResponse`.

**Boundary discipline.** The reasoning trace is **rationale evidence, not part of the artifact's semantic content**. It does not affect SHACL validation of the artifact body. An ADR with a missing required field fails validation regardless of how thorough the reasoning trace is. The worker writes `rationale_trace` as a separate artifact field; the audit role can mine it, the meta-loop can analyze it, reviewers can use it — but the body is what's validated.

**Artifact attachment.** The dispatcher (not this feature) decides which artifacts carry a `rationale_trace` field. For Phase 2, the architect's `ADR` output schema gains an optional `rationale_trace` field; the verifier's `VerificationVerdict` does not (verdict carries `rationale` as a required short justification, distinct from the model's full chain). [FT-061](FT-061) populates the field from `ModelResponse.rationale_trace` when the response carries one.

### State

- No persistent state in the router. Construction per dispatch.
- Tool schemas are module-level constants (the canonical list does not change at runtime).

### Behaviour

1. `build_router(endpoint)` returns either `AnthropicRouter()` or `ScalewayRouter()`. Construction is cheap; callers may cache per worker process if needed.
2. `router.call(system, user, params)`:
   - For Scaleway: forwards to [FT-059](FT-059)'s `scaleway_chat_caller` / `scaleway_tool_caller` / `scaleway_json_caller` depending on which of `params.tools` / `params.response_schema` is set. Passes `reasoning_effort` as a top-level kwarg when set. Sets `rationale_trace` from `message.reasoning` when `params.exposes_reasoning_trace = true`.
   - For Anthropic: constructs an `anthropic.Anthropic().messages.create(...)` call; translates tools; extracts text + tool_uses + usage. Reads cache breakdown from `response.usage.cache_creation_input_tokens` / `cache_read_input_tokens` (zero when not supported by the model). Sets `tokens_cache_write` and `tokens_cache_hit` in the response. Cache breakpoint placement is set by [FT-065](FT-065) on the request payload; the router just reads back the resulting token counts.
3. The response is normalised to `ModelResponse` regardless of endpoint. Callers (worker code) read `text` for the model's reply, `tool_calls` for any tool invocations, `tokens_*` for telemetry, `rationale_trace` for the reasoning chain when present, `stop_reason` for diagnostics.
4. The verifier worker becomes:
   ```python
   router = build_router(dispatch.endpoint)
   params = CallParams(
       endpoint=dispatch.endpoint,
       model_identifier=dispatch.model_identifier,
       max_tokens=bundle.max_tokens,
       reasoning_effort=dispatch.parameters.get("reasoning_effort"),
       response_schema=VerificationVerdict.model_json_schema(),
       exposes_reasoning_trace=dispatch.parameters.get("exposes_reasoning_trace", False),
   )
   response = router.call(SYSTEM_PROMPT, build_user_prompt(bundle), params)
   verdict = VerificationVerdict.model_validate_json(response.text or _extract_tool_input(response))
   # response.rationale_trace, response.tokens_cache_hit etc. surface via the worker's
   # structured result for the harness to record on the session.
   ```

### Invariants

- `ModelRouter.call` is pure with respect to the orchestration graph (no graph reads / writes — [ADR-008](ADR-008)).
- The same `CallParams` produces equivalent semantics across endpoints (the model id changes; the abstract behavior — "call a model with system+user, optionally with tools or schema, return text+usage+optional-trace" — is identical).
- Tool schemas are declared exactly once. Both routers consume the same source list.
- Token counts come from the provider's `usage` block, never inferred locally.
- `ModelResponse.tokens_cache_write` and `tokens_cache_hit` are non-negative integers; both are 0 for Scaleway dispatches.
- `ModelResponse.rationale_trace` is non-`None` only when `params.exposes_reasoning_trace = true` AND the model actually emitted a trace. Absence is the default; absence is non-blocking; the worker does not treat an absent trace as an error.

### Error handling

- Endpoint-specific errors are normalised into a common shape: `ModelRouterError { category, detail }` with categories `auth_failed`, `rate_limited`, `network_error`, `invalid_response`, `unknown`.
- Validation failures (Pydantic, JSON parse) bubble up to the worker; the verifier's existing one-retry-on-validation-failure pattern is preserved.
- Missing `SCW_SECRET_KEY` for a Scaleway router → router construction raises `ScalewayClientError` from [FT-059](FT-059); the worker surfaces this as a session-level error.
- An Anthropic response missing cache-token fields (older API surface) → router sets `tokens_cache_write = 0` and `tokens_cache_hit = 0`, attributing all input to `tokens_in`. The cache-hit rate computed by [FT-057](FT-057) is 0.0 for that session.
- Empty content with non-empty `rationale_trace` (Scaleway long-chain mid-stream case) → the router returns the `ModelResponse` as-is; downstream callers decide. For single-shot synchronous calls this is unusual (response is final by the time we read it); if it happens, the downstream JSON parser will fail and the verifier's existing retry path kicks in.

### Boundaries

- **In scope.** `model_router.py`, `tools.py`, verifier worker refactor, schemas-as-data approach, reasoning-trace ingestion via `getattr`, cache-token extraction from Anthropic responses.
- **Out of scope.** Code-writer worker integration — [FT-064](FT-064) handles the migration of `code-writer` (which today uses `claude -p` agentic subprocess; the move to a `ModelRouter`-based agentic harness is a larger lift handled in the migration feature).
- **Out of scope.** Dispatcher logic — [FT-061](FT-061) / [FT-062](FT-062).
- **Out of scope.** Capability resolution — [FT-061](FT-061).
- **Out of scope.** Setting Anthropic cache breakpoints on the request — [FT-065](FT-065).
- **Out of scope.** Attaching `rationale_trace` to the produced artifact in the graph — that's the artifact-schema decision made by [FT-061](FT-061) (which forwards the worker result to the harness).

## Out of scope

- Anthropic prompt caching — request-side cache breakpoint placement is [FT-065](FT-065); the router only reads back the resulting token-breakdown counts.
- Streaming responses (single-shot single-completion remains the verifier shape per [ADR-020](ADR-020)).
- Per-endpoint retry policy beyond surfacing the error — rate-limit handling is deferred (PRD §13).
- Tool-call schemas for roles that don't exist yet (vision-shaped, sensing) — those roles bind to specialty capabilities but have no worker integration in this slice.
- Anthropic extended thinking (`thinking` parameter on Opus 4.7) — the `rationale_trace` slot can later be populated from `response.content` blocks of type `thinking`, but this requires the Anthropic capabilities to first flip `exposes_reasoning_trace = true` and a pricing decision for extended-thinking token usage. Out of scope here; explicit follow-up.
