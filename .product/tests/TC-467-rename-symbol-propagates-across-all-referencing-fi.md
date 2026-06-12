---
id: TC-467
title: rename_symbol propagates across all referencing files in the worktree
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_180_rename_propagates
runner-timeout: 600
observes:
- disk-state
- file
---

## Description

`rename_symbol` on a fixture symbol referenced from three files: asserts on **disk-state** (all three referencing files plus the declaration site carry the new name; no other files changed) and **file** (the declaration file compiles under the post-rename `get_diagnostics` check — the rename was semantic, not textual, so a same-named string literal in a fixture comment is untouched).
