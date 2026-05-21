---
id: TC-071
title: matcher returns CompleteMultiple deterministic greedy cover with stable tiebreak
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
---

## Premise

Feature `FT-W` references TCs `[T1, T2, T3, T4]`. In env `ENV-1`:

- `VG-3` covers `[T1, T2]` (numeric suffix 3).
- `VG-5` covers `[T2, T3]` (numeric suffix 5).
- `VG-7` covers `[T3, T4]` (numeric suffix 7).
- `VG-9` covers `[T4]` (numeric suffix 9).

A naive greedy cover starting at `VG-5` (covers 2) would then need both `VG-3` and `VG-9` (3 graphs total). A better greedy starting at `VG-3` then `VG-7` covers everything in 2 graphs. The deterministic tiebreak is lowest numeric suffix on the largest-cover-first heuristic.

## Acceptance Criteria

- `best_matching_graphs(FT-W, ENV-1, store)` returns:
  - `kind = MatchKind::CompleteMultiple`,
  - `graphs` is exactly `[VG-3, VG-7]` in that order (lowest-suffix-first when ties; the two-graph cover beats the three-graph cover).
  - `covered_by_match = [T1, T2, T3, T4]`,
  - `residual_uncovered = []`.
- The result is **deterministic** — running the matcher 100 times produces identical ordered output.

## Notes

This is the contract that makes the worker's bundle deterministic across runs and the chain gate's behaviour reproducible. If the matcher were non-deterministic, two CI runs over the same store could produce different `Match`/`Gap` outcomes.
