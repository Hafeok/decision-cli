---
id: TC-334
title: dec product adr show ADR-009 surfaces the adr title and status
type: scenario
status: passing
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/dec-product-verb.sh adr show ADR-009
runner-timeout: 60
observes:
- stdout
- exit-code
last-run: 2026-06-03T12:09:07.168277302+00:00
last-run-duration: 0.1s
---

## Acceptance criteria

Verifies that `dec product adr show ADR-009` (as wired in [FT-136](FT-136) §Phase 2) loads the KnowledgeGraph and surfaces the requested ADR.

### Conditions

- Run `dec product adr show ADR-009` against this repo's `.product/`.
- Exits with code `0`.
- stdout contains the literal string `ADR-009`.
- stdout contains a recognisable substring of the ADR title (`product-cli` or `subprocess` — the actual ADR-009 title is "product-cli integration via subprocess and MCP for slice-1").
- Running `dec product adr show ADR-NONEXISTENT` exits non-zero with a stderr diagnostic.

### Surface

`stdout`, `exit-code` — bash script.