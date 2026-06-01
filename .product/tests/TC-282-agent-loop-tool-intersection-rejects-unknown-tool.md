---
id: TC-282
title: 'agent loop: tool intersection drops unknown tool names with a warn log'
type: scenario
status: unimplemented
validates:
  features:
  - FT-123
  adrs:
  - ADR-069
  - ADR-070
phase: 4
observes:
- exit-code
- stderr
runner: pytest
runner-args: workers/code-writer/tests/test_agent_loop.py::test_unknown_tool_names_dropped
runner-timeout: 30
---

## Description

`payload.allowed_tools` may contain a tool name the current worker doesn't yet know about — e.g. an in-flight catalog migration where a new tool was seeded before the worker shipped its implementation. The loop's contract per [ADR-069](ADR-069) is: intersect the catalog list with the registry, drop unknowns with a warn log, proceed.

This is forward-compatibility: an operator who edits the role-catalog seed to add `apply_patch` should not break old workers that still ship without that tool. The dropped name is logged for operator visibility; the loop continues with the intersection.

This is distinct from TC-271 (FT-122): that asserts the empty-intersection fail-closed path. This TC asserts the partial-intersection succeed path.

## Acceptance Criteria

Pytest test at `workers/code-writer/tests/test_agent_loop.py::test_unknown_tool_names_dropped`.

Setup:

- `DispatchPayload` with `allowed_tools=["read_file", "write_file", "apply_patch"]`. Note: `apply_patch` is NOT in `TOOL_REGISTRY` for this worker version.
- `litellm.completion` patched; captures `kwargs`.
- `caplog` fixture captures log records.

Assertions:

- `run_agent(payload).status == "ok"`.
- The patched `litellm.completion`'s `tools=` kwarg contains exactly two tool schemas — `read_file` and `write_file`. `apply_patch` is absent.
- `caplog.records` contains at least one record at WARN level whose message references `"apply_patch"` and the word `"unknown"` or `"unregistered"` (operator readability).
- The intersection happens BEFORE the first `litellm.completion` call — the patched call sees the filtered list, not the raw payload list.
