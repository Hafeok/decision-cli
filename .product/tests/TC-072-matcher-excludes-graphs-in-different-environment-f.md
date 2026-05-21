---
id: TC-072
title: matcher excludes graphs in different environment from match set
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 2
---

## Premise

Feature `FT-V` references TCs `[T1, T2]`. Two graphs exist:

- `VG-A` in `ENV-1` covers `[T1]` only.
- `VG-B` in `ENV-2` covers `[T1, T2]`.

A naive global matcher would prefer `VG-B`. The per-env matcher must not.

## Acceptance Criteria

- `best_matching_graphs(FT-V, ENV-1, store)` returns:
  - `kind = MatchKind::Partial`,
  - `graphs = [VG-A]`,
  - `covered_by_match = [T1]`,
  - `residual_uncovered = [T2]`.
- `VG-B` is never returned for an `ENV-1` query, regardless of how complete its coverage is in its own env.

## Notes

Per-env scoping is the slice 2.6 contract; cross-env composite matching is explicitly out of scope per [FT-046](FT-046). This test pins that boundary.
