---
id: TC-464
title: get_diagnostics reports a broken fixture edit in-session without run_build, and absent code_intel_url degrades to textual tools with telemetry record
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: pytest
runner-args: workers/code-writer/tests/test_ft_179_diagnostics_and_degrade.py
runner-timeout: 300
observes:
- stdout
- exit-code
---

## Description

Two halves, pytest worker-side with a stub code-intel endpoint: (1) after writing a deliberately broken edit to the fixture, `get_diagnostics` returns the compiler error for that file in the same worker session, and the recorded telemetry shows no `run_build` invocation; (2) with semantic tools granted but `code_intel_url` absent from the payload, the exposed tool registry omits the semantic tools, the dispatch completes textually (**exit-code**), and the telemetry records the degradation (**stdout** — the structured worker response carries the omission note).
