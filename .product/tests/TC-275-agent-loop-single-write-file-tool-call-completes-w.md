---
id: TC-275
title: 'agent loop: single write_file tool call completes with status=ok and one FileWrite'
type: scenario
status: unimplemented
validates:
  features:
  - FT-123
  adrs:
  - ADR-069
phase: 4
observes:
- disk-state
- exit-code
runner: pytest
runner-args: workers/code-writer/tests/test_agent_loop.py::test_single_write_completes
runner-timeout: 60
---

## Description

The happy-path end-to-end assertion against a mocked LiteLLM. The model emits one `tool_use(write_file)` block, then `end_turn`. The loop dispatches the tool, the file lands in the workspace, and the response surfaces `WorkerResponse.status="ok"` with one `FileWrite` recorded.

## Acceptance Criteria

Pytest test at `workers/code-writer/tests/test_agent_loop.py::test_single_write_completes`.

Setup:

- `tmp_path / "workspace"` is created, empty.
- A `DispatchPayload` is constructed with `allowed_tools=["read_file", "write_file"]`, `workspace_path=tmp_path / "workspace"`, `endpoint="anthropic"`, `model_id="claude-sonnet-4-5"`, `max_turns=4`.
- `litellm.completion` is patched. On call 1 it returns a mock response with one assistant message containing `tool_use(name="write_file", input={"path": "hello.py", "content": "print('hi')\n", "old_string": None})`, `stop_reason="tool_use"`. On call 2 it returns `stop_reason="end_turn"` with assistant text `"Created hello.py."`.

Assertions:

- `run_agent(payload).status == "ok"`.
- `(tmp_path / "workspace" / "hello.py").read_text() == "print('hi')\n"` (disk-state observation).
- The response's `file_writes` list has length 1; its single entry's `path == "hello.py"`.
- The response's `tool_calls` telemetry contains exactly one entry with `name="write_file"`, `status="ok"`.
- The patched `litellm.completion` was called exactly twice.
