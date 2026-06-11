---
id: TC-424
title: Archetype negative SHACL set — invalid status, missing application contract, missing provenance all rejected
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
runner-args: -p dec-ontology -- invalid_status_fails_shacl missing_application_contract_fails_shacl missing_provenance_fails_parse
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T17:46:12.116126695+00:00
last-run-duration: 0.2s
---

## Purpose

FT-147 spec test cases 3–4 plus the FT-073 provenance discipline: the remaining negative set against the pure archetype module.

1. `dec:status` outside `candidate | standard | quarantined` is rejected (sh:in).
2. A missing `dec:applicationContract` link is rejected (sh:minCount 1).
3. Quads without the mechanical provenance block fail to parse (dual provenance is mandatory per FT-072/FT-073).

## Mechanism

`cargo test -p dec-ontology invalid_status_fails_shacl missing_application_contract_fails_shacl missing_provenance_fails_parse` — three targeted tests in `crates/dec-ontology/src/ontology/archetype/tests.rs`.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — all three rejections fire with field-naming reports.

## Fail criteria

Exit-code non-zero; stdout names the constraint that regressed.