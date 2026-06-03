---
id: TC-339
title: dec product graph stats reports artifact and edge counts
type: scenario
status: passing
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/dec-product-verb.sh graph stats
runner-timeout: 60
observes:
- stdout
- exit-code
last-run: 2026-06-03T12:09:07.168277302+00:00
last-run-duration: 0.5s
---

## Acceptance criteria

Verifies that `dec product graph stats` (as wired in [FT-136](FT-136) §Phase 2) calls `KnowledgeGraph::stats()` and renders the artifact/edge counts.

### Conditions

- Run `dec product graph stats` against this repo's `.product/`.
- Exits with code `0`.
- stdout contains numeric output (at least one digit run).
- stdout references at least one artifact type (substring `features`, `adrs`, `tests`, or `patterns`).

### Surface

`stdout`, `exit-code` — bash script.