---
id: TC-069
title: coverage primitive treats absent providesEvidenceFor as no coverage
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 2
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
