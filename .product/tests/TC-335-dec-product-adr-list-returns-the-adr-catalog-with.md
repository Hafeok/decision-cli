---
id: TC-335
title: dec product adr list returns the adr catalog with status and scope
type: scenario
status: passing
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/dec-product-verb.sh adr list
runner-timeout: 60
observes:
- stdout
- exit-code
last-run: 2026-06-03T12:09:07.168277302+00:00
last-run-duration: 0.1s
---

## Acceptance criteria

Verifies that `dec product adr list` (as wired in [FT-136](FT-136) §Phase 2) renders the full ADR catalog.

### Conditions

- Run `dec product adr list` against this repo's `.product/`.
- Exits with code `0`.
- stdout contains at least one ID matching `ADR-\d{3}`.
- stdout contains a status indicator (substring matching one of `accepted`, `proposed`, `superseded`, `abandoned`).

### Surface

`stdout`, `exit-code` — bash script.