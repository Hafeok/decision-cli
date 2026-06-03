---
id: TC-332
title: dec product feature list returns the feature catalog with phase and status
type: scenario
status: unimplemented
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/dec-product-verb.sh feature list
runner-timeout: 60
observes:
- stdout
- exit-code
---

## Acceptance criteria

Verifies that `dec product feature list` (as wired in [FT-136](FT-136) §Phase 2) loads the KnowledgeGraph via `product_core::graph::KnowledgeGraph::build_full(repo_root)` and renders the feature catalog.

### Conditions

- Run `dec product feature list` against this repo's `.product/`.
- Exits with code `0`.
- stdout contains at least one feature ID matching the pattern `FT-\d{3}` (e.g. `FT-001`).
- stdout contains a phase indicator (substring `phase` or column header — exact format up to renderer).
- stdout contains a status indicator (substring matching one of `planned`, `in-progress`, `complete`).

### Surface

`stdout`, `exit-code` — bash script invokes the `dec` binary and asserts via grep.
