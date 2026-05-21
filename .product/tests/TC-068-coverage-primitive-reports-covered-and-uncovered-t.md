---
id: TC-068
title: coverage primitive reports covered and uncovered TCs structurally
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
---

## Premise

A feature `FT-X` references TCs `[T1, T2, T3]`. A graph `VG-X` in env `ENV-1` contains two steps: step-1 has `dec:providesEvidenceFor T1`, step-2 has `dec:providesEvidenceFor T2`. T3 is referenced by no step in any graph.

## Acceptance Criteria

- `feature_covered_by(FT-X, VG-X, store)` returns a `CoverageReport` with:
  - `all_tcs = [T1, T2, T3]`,
  - `covered = [(T1, VG-X, step-1), (T2, VG-X, step-2)]`,
  - `uncovered = [T3]`,
  - `considered = [VG-X]`.
- `feature_coverage(FT-X, None, store)` (no explicit candidate set) returns the same `uncovered = [T3]` after considering every graph in the store.
- The primitive performs no writes to the store or disk.
- Running the primitive twice over the same store snapshot returns byte-equal reports.

## Notes

Validates the core promise of [ADR-030](ADR-030)'s coverage definition: structural via `dec:providesEvidenceFor`, queryable, deterministic. The test seeds the store via [FT-036](FT-036)'s round-trip API and runs the SPARQL query through the [FT-045](FT-045) primitive.
