---
id: TC-422
title: Archetype E102 — empty seam-audit set rejected by the pure validator (ADR-084 §1)
type: invariant
status: passing
validates:
  features:
  - FT-147
  adrs:
  - ADR-082
  - ADR-084
  - ADR-085
phase: 1
runner: cargo-test
runner-args: -p dec-ontology empty_seam_audits_fails_shacl_with_e102
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T17:46:12.116126695+00:00
last-run-duration: 0.1s
---

## Purpose

FT-147 spec test case 2 / ADR-084 §1: an archetype with an empty seam-audit set is the one decomposition strictly worse than the broad-worker baseline, so the pure validator must reject it with `E102_ArchetypeMissingSeamAudits`.

## Mechanism

`cargo test -p dec-ontology empty_seam_audits_fails_shacl_with_e102` — builds the positive fixture, clears `seam_audits`, asserts `validate_quads` rejects and the report carries the E102 code.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — the validator rejected with E102.

## Fail criteria

Exit-code non-zero — the empty seam-audit set was accepted or rejected without the E102 code.