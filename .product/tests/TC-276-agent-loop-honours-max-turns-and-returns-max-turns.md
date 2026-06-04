---
id: TC-276
title: 'agent loop: honours max_turns and returns max_turns_exceeded with partial telemetry'
type: scenario
status: passing
validates:
  features:
  - FT-123
  adrs:
  - ADR-069
phase: 4
observes:
- exit-code
runner: pytest
runner-args: workers/code-writer/tests/test_agent_loop.py::test_max_turns_exceeded
runner-timeout: 60
last-run: 2026-06-04T12:26:46.082720237+00:00
last-run-duration: 0.3s
---

## Description

The loop must not run forever. When the model keeps issuing `tool_use` blocks past `payload.max_turns`, the loop terminates with a structured error and the partial telemetry survives so the harness (and the operator) can see what the model attempted.

## Acceptance Criteria

Pytest test at `workers/code-writer/tests/test_agent_loop.py::test_max_turns_exceeded`.

Setup:

- `DispatchPayload` with `max_turns=2`, `allowed_tools=["read_file", "write_file"]`, fresh workspace.
- `litellm.completion` patched to always return `stop_reason="tool_use"` with a single `tool_use(name="read_file", input={"path": "README.md"})` block. Workspace has a README so the tool succeeds — but the model never says `end_turn`.

Assertions:

- `run_agent(payload).status == "error"`.
- `response.error.category == "max_turns_exceeded"`.
- `litellm.completion` was called exactly `max_turns` times (2).
- `response.tool_calls` has length 2 (one per turn — telemetry is preserved across the failure).
- No file in the workspace was modified (only `read_file` was invoked).