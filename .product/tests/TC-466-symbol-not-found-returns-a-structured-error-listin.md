---
id: TC-466
title: symbol-not-found returns a structured error listing the symbols available in the file
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: pytest
runner-args: workers/code-writer/tests/test_ft_180_symbol_not_found.py
runner-timeout: 300
observes:
- stdout
---

## Description

Calling a symbol-level tool with a symbol name absent from the target file must return a structured error that lists the symbols actually present in the file (the factory's one-turn self-correction contract). Asserts on **stdout** (the recorded tool result is an error block carrying the available-symbols list; it is not an exception, not a silent no-op, and not a fuzzy match applied to the wrong symbol).
