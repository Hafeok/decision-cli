---
id: TC-277
title: 'agent loop: no model-invoking subprocess fires during a dispatch'
type: scenario
status: unimplemented
validates:
  features:
  - FT-123
  adrs:
  - ADR-069
phase: 4
observes:
- exit-code
runner: pytest
runner-args: workers/code-writer/tests/test_agent_loop.py::test_no_model_subprocess
runner-timeout: 60
---

## Description

[ADR-069](ADR-069) explicitly bans subprocess-based model invocation in the worker. This TC is the boundary test: it asserts that a normal dispatch (with `allowed_tools` restricted to `read_file` and `write_file` — no `run_*` tools) completes without ANY subprocess spawning. The presence of even one would indicate the `claude -p` path snuck back in.

The companion property — that the `run_*` tools DO legitimately spawn subprocesses when invoked — is asserted separately by the FT-124 / FT-123 subprocess containment tests; those use the real `subprocess` API.

## Acceptance Criteria

Pytest test at `workers/code-writer/tests/test_agent_loop.py::test_no_model_subprocess`.

Setup:

- `DispatchPayload` with `allowed_tools=["read_file", "write_file"]` (the non-subprocess tools).
- `subprocess.Popen` and `subprocess.run` are patched at module level to raise `AssertionError("subprocess invoked during model-only dispatch")`.
- `litellm.completion` patched to return one assistant text turn ending with `stop_reason="end_turn"`.

Assertions:

- `run_agent(payload)` completes without raising.
- `response.status == "ok"`.
- The `subprocess.Popen` and `subprocess.run` patches were never called (mock `assert_not_called()` on both).

This is a regression guard against accidental reintroduction of the `claude -p` path. If any future PR adds a subprocess-based shortcut for model calls, this test fails immediately.
