---
id: TC-106
title: ModelRouter routes Scaleway and Anthropic uniformly with tool-call translation
type: exit-criteria
status: passing
validates:
  features:
  - FT-060
  adrs: []
phase: 2
runner: pytest
runner-args: workers/_shared/tests/test_model_router.py
runner-timeout: 60
last-run: 2026-05-23T18:00:11.823551315+00:00
last-run-duration: 0.5s
---

## Description

Scenario: `ModelRouter` from [FT-060](FT-060) routes uniformly to either Anthropic or Scaleway based on `CallParams.endpoint`, translates the canonical OpenAI function-tools format to Anthropic tool-use at the Anthropic boundary, and returns a normalised `ModelResponse` regardless of endpoint.

The runner is `pytest`. Tests use stub endpoints (no live calls) to assert the routing and translation contracts.

Acceptance:

1. **Endpoint routing.** `build_router("scaleway")` returns a `ScalewayRouter`; `build_router("anthropic")` returns an `AnthropicRouter`. `build_router("invalid")` raises `ModelRouterError(category="unknown")`.
2. **Uniform response shape.** Stub each router to return canned content + 1 tool call + usage. Call `router.call("sys", "user", params)` with the same `CallParams` (modulo endpoint). Assert both routers return a `ModelResponse` with the same field names and types (`text: str`, `tool_calls: list[ToolCall]`, `tokens_in: int`, `tokens_out: int`, `stop_reason: str`).
3. **Tool schema translation.** Define a canonical OpenAI tool: `{type: "function", function: {name: "edit_file", description: "…", parameters: {…}}}`. Inject this into `CallParams.tools`. For the Anthropic router, intercept the SDK call; assert the tool is translated to `{name: "edit_file", description: "…", input_schema: {…}}`. For the Scaleway router, intercept; assert the tool is passed through unchanged.
4. **Structured output, Scaleway path.** Set `CallParams.response_schema = VerificationVerdict.model_json_schema()`. Intercept the Scaleway call; assert `response_format = {"type": "json_schema", "json_schema": {"name": "Verdict", "schema": <…>, "strict": true}}`.
5. **Structured output, Anthropic path.** Same `response_schema`. Intercept; assert a single tool named `submit_verdict` is declared with `input_schema = <schema>` and `tool_choice = {type: "tool", name: "submit_verdict"}`.
6. **reasoning_effort passthrough.** Set `CallParams.reasoning_effort = "medium"`. Intercept Scaleway call; assert the parameter is forwarded (exact placement — top-level or `extra_body` — per [FT-059](FT-059) empirical resolution). For Anthropic router, assert the parameter is silently ignored (no error).
7. **Error normalisation.** Stub the Anthropic SDK to raise an auth-failure exception. Assert it surfaces as `ModelRouterError(category="auth_failed")`. Repeat for `rate_limited`, `network_error`, `invalid_response`.
8. **Verifier worker uses router.** Run the verifier worker against a stub router for endpoint `scaleway`. Assert the worker constructs `CallParams(endpoint="scaleway", model_identifier=<from dispatch>, …)` — no string constants for the model id.

⟦Σ:Types⟧{
  Endpoint ≜ scaleway | anthropic
  CallParams ≜ ⟨endpoint:Endpoint, modelId:String, maxTokens:Nat, temperature:Real, reasoningEffort:Maybe (low | medium | high), tools:Maybe List Tool, responseSchema:Maybe JSONSchema⟩
  ModelResponse ≜ ⟨text:String, toolCalls:List ToolCall, tokensIn:Nat, tokensOut:Nat, stopReason:String⟩
}

⟦Γ:Invariants⟧{
  ∀ p:CallParams: shape_of(router(p.endpoint).call(_, _, p)) = ModelResponse
  ∀ tool: anthropic_translate(openai_tool(tool)).input_schema = tool.function.parameters
}