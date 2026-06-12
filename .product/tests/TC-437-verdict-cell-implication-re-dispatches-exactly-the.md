---
id: TC-437
title: verdict cell implication re-dispatches exactly the named cells
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_173_verdict_cell_implication
runner-timeout: 300
observes:
- graph
- disk-state
---

## Description

A fixture cluster run uses a stub audit declared `verdict: v1` that exits 1 and writes a `dec-audit-verdict/v1` document whose single failed check names one cell in `implicates.cells`. The repair round must re-dispatch exactly that cell: the test asserts on **disk-state** (only the implicated cell's `output_path` in the sandbox is replaced; sibling cells' outputs keep their pre-repair content/mtimes) and on the **graph** (the persisted cluster SessionRecord shows a repair attempt for the implicated cell only, with no degradation flag).
