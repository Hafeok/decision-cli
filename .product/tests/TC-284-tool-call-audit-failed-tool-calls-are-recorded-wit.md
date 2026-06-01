---
id: TC-284
title: 'tool-call audit: failed tool calls are recorded with toolStatus=error'
type: scenario
status: unimplemented
validates:
  features:
  - FT-125
  adrs:
  - ADR-050
  - ADR-071
phase: 4
observes:
- graph
runner: pytest
runner-args: workers/code-writer/tests/test_audit.py::test_failed_tool_call_recorded_with_error_status
runner-timeout: 30
---

## Description

Tool failures (containment violations, secrets-blocked writes, subprocess timeouts) must surface in the audit trail — not be silently dropped. This is the "failure visibility" property: an operator inspecting a dispatch should see *what the model attempted* even when the attempts failed.

The Python side of the contract: the worker's `tool_call_audit` list must contain an entry for every tool invocation, including the failed ones, with `status="error"`. The Rust side (TC-283) handles the persistence; this TC pins the producer.

## Acceptance Criteria

Pytest test at `workers/code-writer/tests/test_audit.py::test_failed_tool_call_recorded_with_error_status`.

Setup:

- `tmp_path / "workspace"` is the workspace.
- `DispatchPayload` with `allowed_tools=["read_file", "write_file"]`.
- `litellm.completion` patched. Call 1 returns `tool_use(name="write_file", input={"path": ".env", "content": "X"})` — this WILL be blocked by `is_write_blocked`. Call 2 returns `tool_use(name="write_file", input={"path": "../escape", "content": "X"})` — blocked by `safe_join`. Call 3 returns `end_turn` with assistant text `"OK, can't write those."`.

Assertions:

- `response.status == "ok"` — the dispatch succeeds; tool errors are surfaced to the model but don't terminate the loop (model recovered).
- `response.tool_call_audit` has length 2 — one entry per blocked attempt.
- Both audit entries have `status == "error"`.
- Audit entries have populated `args_hash`, `started_at`, `ended_at` even though the tool failed (timing is recorded regardless of success).
- `(tmp_path / "workspace" / ".env")` does NOT exist (containment held).
- No file outside the workspace was created.

This is the property that makes the audit usable: failed calls are first-class citizens in the trail, equal to successful ones.
