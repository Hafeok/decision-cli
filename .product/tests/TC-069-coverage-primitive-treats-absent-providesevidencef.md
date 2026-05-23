---
id: TC-069
title: coverage primitive treats absent providesEvidenceFor as no coverage
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test tc_069_coverage_primitive_treats_absent_providesevidencef
runner-timeout: 120
last-run: 2026-05-23T17:59:57.707512474+00:00
last-run-duration: 0.2s
---

## Premise

A graph `VG-Y` in env `ENV-1` contains steps that have **no** `dec:providesEvidenceFor` triples — valid per [FT-036](FT-036)'s optional-predicate rule. The graph references feature `FT-Y` via `dec:verifies` but provides no per-step TC linkage.

## Acceptance Criteria

- `feature_coverage(FT-Y, Some([VG-Y]), store)` returns:
  - `covered = []`,
  - `uncovered = FT-Y.tests` (everything is uncovered).
- The primitive does **not** error on the absent predicate — it treats absence as "this step covers no TC".
- SHACL validation of `VG-Y` continues to succeed (the optional-predicate rule from [FT-036](FT-036) is preserved).

## Notes

Forward-compat check: pre-slice-2.6 graphs (or partially-annotated graphs) must remain SHACL-valid and produce coherent (zero-coverage) reports rather than crashes or warnings.