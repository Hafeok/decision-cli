---
id: TC-333
title: dec product feature next returns the next implementable feature
type: scenario
status: passing
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/dec-product-verb.sh feature next
runner-timeout: 60
observes:
- stdout
- exit-code
last-run: 2026-06-03T12:09:07.168277302+00:00
last-run-duration: 0.1s
---

## Acceptance criteria

Verifies that `dec product feature next` (as wired in [FT-136](FT-136) §Phase 2) returns the next implementable feature using `product_core`'s dependency-ordering logic.

### Conditions

- Run `dec product feature next` against this repo's `.product/`.
- Exits with code `0` (or `0` when nothing is implementable — the absence of next is not an error).
- If a feature is returned, stdout contains a feature ID matching `FT-\d{3}`.

### Surface

`stdout`, `exit-code` — bash script.