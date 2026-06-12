---
id: TC-465
title: replace_symbol_body edits exactly the named symbol range and post-edit diagnostics reflect the change
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_180_replace_symbol_body
runner-timeout: 600
observes:
- file
- stdout
---

## Description

`replace_symbol_body` against a named fn in a fixture crate: asserts on **file** (only the symbol's range changed — surrounding code is byte-identical, verified by diffing against the pre-edit content outside the symbol's lines) and **stdout** (the tool result reports the resolved symbol, range, and lines_changed; a follow-up `get_diagnostics` in the same session reflects the post-edit state, stale-free).
