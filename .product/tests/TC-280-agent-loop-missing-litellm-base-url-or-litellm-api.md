---
id: TC-280
title: 'agent loop: missing LITELLM_API_KEY returns structured invalid_dispatch error'
type: scenario
status: passing
validates:
  features:
  - FT-123
  adrs:
  - ADR-053
  - ADR-069
phase: 4
observes:
- exit-code
runner: pytest
runner-args: workers/code-writer/tests/test_provider_routing.py::test_missing_litellm_key_fails_closed
runner-timeout: 30
last-run: 2026-06-04T12:25:32.225732911+00:00
last-run-duration: 0.4s
---

## Description

[ADR-053](ADR-053) names `LITELLM_API_KEY` as required. If the worker starts up against an env that omits it (or sets it to an empty string), the loop must fail with a structured `invalid_dispatch` error pre-LLM-call — not a stack trace from inside the SDK.

`LITELLM_BASE_URL` has a default per [ADR-053](ADR-053) (`http://localhost:4000`), so its absence is NOT a failure. Only the key is required-without-default.

## Acceptance Criteria

Pytest test at `workers/code-writer/tests/test_provider_routing.py::test_missing_litellm_key_fails_closed`.

**Case A — LITELLM_API_KEY unset:**

- Env has `LITELLM_BASE_URL=http://proxy.test:4000` but no `LITELLM_API_KEY`.
- `litellm.completion` patched to raise `AssertionError("should not be called")`.
- Call `run_agent(payload)`.
- Assert `response.status == "error"`.
- Assert `response.error.category == "invalid_dispatch"`.
- Assert the error message references `LITELLM_API_KEY` (operator readability).
- Assert the patched `litellm.completion` was never called.

**Case B — LITELLM_API_KEY empty string:**

- Same setup as Case A but with `LITELLM_API_KEY=""` (whitespace-stripped to empty).
- Same assertions — empty string treated identically to absent.

**Case C — LITELLM_BASE_URL absent but key present:**

- Env has `LITELLM_API_KEY=sk-virtual-001` but no `LITELLM_BASE_URL`.
- `litellm.completion` patched to capture kwargs.
- Call `run_agent(payload)`.
- Assert `litellm.completion` was called with `base_url="http://localhost:4000"` (the [ADR-053](ADR-053) default).
- Assert `response.status == "ok"` — absent base URL is NOT a failure, it falls back.