---
id: TC-271
title: Worker fail-closes with invalid_dispatch error when allowed_tools is empty
type: scenario
status: passing
validates:
  features:
  - FT-122
  - FT-123
  adrs:
  - ADR-069
  - ADR-070
phase: 4
observes:
- stdout
- exit-code
runner: pytest
runner-args: workers/code-writer/tests/test_allowed_tools_fail_closed.py
runner-timeout: 30
last-run: 2026-06-04T11:33:49.027021278+00:00
last-run-duration: 0.7s
---

## Description

The fail-closed contract from [ADR-069](ADR-069). A worker that receives a payload with `allowed_tools: []` MUST refuse the dispatch before any LLM call, returning a structured `WorkerResponse` with `status="error"` and `error.category="invalid_dispatch"`. This TC asserts the property at the Python worker layer.

The test runs against the worker as a unit (no Rust harness involvement) — feed a payload directly into the dispatch entrypoint, observe the response. This is the contract test that lets FT-122 / FT-121 / FT-123 be re-ordered without losing the safety property.

## Acceptance Criteria

Pytest test at `workers/code-writer/tests/test_allowed_tools_fail_closed.py::test_empty_allowed_tools_returns_invalid_dispatch`.

Construct a `DispatchPayload` with all required fields populated normally, except `allowed_tools=[]`. Pass it into the worker's dispatch entrypoint (post-FT-123 this is `agent.run_agent(payload)`; pre-FT-123 the entrypoint may not exist yet — the TC is marked dependent on FT-123 in `validates.features`).

Assert:

- The returned `WorkerResponse.status` is `"error"`.
- `WorkerResponse.error.category` is `"invalid_dispatch"`.
- `WorkerResponse.error.message` contains the substring `"no tools granted"` (or equivalent — the message must name the failure mode).
- No `litellm.completion` call occurred during the dispatch (mock the litellm client; assert call count is 0).
- No file writes happened (workspace_path content unchanged).

For the parallel case where `allowed_tools` contains *only unknown names* (e.g. `["mystery_tool"]`), the same fail-closed behaviour applies — after intersection with the worker's tool registry, the effective set is empty, so the dispatch is refused. Add a second test method asserting this branch.