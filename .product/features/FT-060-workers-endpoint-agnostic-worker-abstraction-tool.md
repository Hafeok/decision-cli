---
id: FT-060
title: 'workers: Endpoint-agnostic worker abstraction (tool schemas and structured output across Scaleway and Anthropic)'
phase: 2
status: planned
depends-on:
- FT-059
adrs:
- ADR-008
- ADR-033
tests:
- TC-106
domains:
- api
- error-handling
domains-acknowledged: {}
---

## Description

Replace the worker-layer endpoint-coupling with a uniform `ModelCaller` abstraction that routes to either the Anthropic SDK or the Scaleway OpenAI-compatible client from [FT-059](FT-059), based on the resolved capability in the dispatch payload. The abstraction lives in `workers/_shared/` so every worker reuses the same shape. Tool schemas are declared once in OpenAI function-tools format and translated to Anthropic's tool-use format at call time; structured outputs use a JSON schema derived from the worker's Pydantic output model.

This is the worker-side seam that closes the loop: dispatcher resolves capability → injects `(endpoint, model, params)` into dispatch payload → worker `ModelCaller` dispatches to the right client. Workers stay stateless and free of endpoint policy ([ADR-008](ADR-008), [ADR-033](ADR-033)).

## Functional Specification

### Inputs

- The existing `ModelCaller` callable in `workers/verifier/src/verifier/worker.py:52` (signature: `(system, user, model_id, max_tokens) -> (raw_json_string, tokens_in, tokens_out)`).
- The Scaleway client wrapper from [FT-059](FT-059).
- The Anthropic SDK already wired in the verifier (`call_claude` at `worker.py:66`).
- The dispatch payload from [FT-061](FT-061), which now carries `endpoint`, `model_identifier`, and `parameters` (incl. optional `reasoning_effort`, `temperature`, `tool_definitions`, `response_schema`).
- The PRD's tool list for the implementer (`read_file`, `edit_file`, `run_bash`, `record_emergent_judgment`, `file_feedback`, `submit`) authored once in OpenAI function-tools format.

### Outputs

- New module `workers/_shared/src/_shared/model_router.py`:
  ```python
  from dataclasses import dataclass
  from typing import Literal, Protocol
  
  Endpoint = Literal["scaleway", "anthropic"]
  
  @dataclass
  class CallParams:
      endpoint: Endpoint
      model_identifier: str
      max_tokens: int
      temperature: float = 0.0
      reasoning_effort: Literal["low", "medium", "high"] | None = None
      tools: list[dict] | None = None         # OpenAI function-tools format (canonical)
      response_schema: dict | None = None     # JSON schema for structured output
  
  class ModelRouter(Protocol):
      def call(self, system: str, user: str, params: CallParams) -> "ModelResponse": ...
  
  @dataclass
  class ModelResponse:
      text: str
      tool_calls: list["ToolCall"]
      tokens_in: int
      tokens_out: int
      stop_reason: str
  
  def build_router(endpoint: Endpoint) -> ModelRouter: ...
  ```
- Anthropic implementation: translates OpenAI tool format → Anthropic `tool_use` format at call time; reads `response.content` for text and `tool_use` blocks for tool calls.
- Scaleway implementation: uses [FT-059](FT-059)'s callers; passes OpenAI tools directly; honours `reasoning_effort` for `configurable_effort` capabilities.
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

### State

- No persistent state in the router. Construction per dispatch.
- Tool schemas are module-level constants (the canonical list does not change at runtime).

### Behaviour

1. `build_router(endpoint)` returns either `AnthropicRouter()` or `ScalewayRouter()`. Construction is cheap; callers may cache per worker process if needed.
2. `router.call(system, user, params)`:
   - For Scaleway: forwards to [FT-059](FT-059)'s `scaleway_chat_caller` / `scaleway_tool_caller` / `scaleway_json_caller` depending on which of `params.tools` / `params.response_schema` is set.
   - For Anthropic: constructs an `anthropic.Anthropic().messages.create(...)` call; translates tools; extracts text + tool_uses + usage.
3. The response is normalised to `ModelResponse` regardless of endpoint. Callers (worker code) read `text` for the model's reply, `tool_calls` for any tool invocations, `tokens_in/out` for telemetry, `stop_reason` for diagnostics.
4. The verifier worker becomes:
   ```python
   router = build_router(dispatch.endpoint)
   params = CallParams(
       endpoint=dispatch.endpoint,
       model_identifier=dispatch.model_identifier,
       max_tokens=bundle.max_tokens,
       reasoning_effort=dispatch.parameters.get("reasoning_effort"),
       response_schema=VerificationVerdict.model_json_schema(),
   )
   response = router.call(SYSTEM_PROMPT, build_user_prompt(bundle), params)
   verdict = VerificationVerdict.model_validate_json(response.text or _extract_tool_input(response))
   ```

### Invariants

- `ModelRouter.call` is pure with respect to the orchestration graph (no graph reads / writes — [ADR-008](ADR-008)).
- The same `CallParams` produces equivalent semantics across endpoints (the model id changes; the abstract behavior — "call a model with system+user, optionally with tools or schema, return text+usage" — is identical).
- Tool schemas are declared exactly once. Both routers consume the same source list.
- Token counts come from the provider's `usage` block, never inferred locally.

### Error handling

- Endpoint-specific errors are normalised into a common shape: `ModelRouterError { category, detail }` with categories `auth_failed`, `rate_limited`, `network_error`, `invalid_response`, `unknown`.
- Validation failures (Pydantic, JSON parse) bubble up to the worker; the verifier's existing one-retry-on-validation-failure pattern is preserved.
- Missing `SCW_SECRET_KEY` for a Scaleway router → router construction raises `ScalewayClientError` from [FT-059](FT-059); the worker surfaces this as a session-level error.

### Boundaries

- **In scope.** `model_router.py`, `tools.py`, verifier worker refactor, schemas-as-data approach.
- **Out of scope.** Code-writer worker integration — [FT-064](FT-064) handles the migration of `code-writer` (which today uses `claude -p` agentic subprocess; the move to a `ModelRouter`-based agentic harness is a larger lift handled in the migration feature).
- **Out of scope.** Dispatcher logic — [FT-061](FT-061) / [FT-062](FT-062).
- **Out of scope.** Capability resolution — [FT-061](FT-061).
- **Out of scope.** New worker types (the contract remains bundle-in/artifact-out per [ADR-008](ADR-008)).

## Out of scope

- Anthropic prompt caching — the existing verifier code does not use prompt caching; adding it is a separate Phase 3 optimisation.
- Streaming responses (single-shot single-completion remains the verifier shape per [ADR-020](ADR-020)).
- Per-endpoint retry policy beyond surfacing the error — rate-limit handling is deferred (PRD §13).
- Tool-call schemas for roles that don't exist yet (vision-shaped, sensing) — those roles bind to specialty capabilities but have no worker integration in this slice.
