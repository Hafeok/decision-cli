---
id: TC-070
title: matcher returns CompleteSingle when one graph covers all feature TCs in env
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: tc_070_matcher_returns_completesingle_when_one_graph_cove
runner-timeout: 120
last-run: 2026-05-23T17:59:58.083061915+00:00
last-run-duration: 0.4s
---

## Premise

Feature `FT-Z` references TCs `[T1, T2, T3]`. A single graph `VG-Z` in env `ENV-1` has steps that collectively declare `dec:providesEvidenceFor` for all three TCs. A second graph `VG-Z2` exists in the same env but covers only `T1`.

## Acceptance Criteria

- `best_matching_graphs(FT-Z, ENV-1, store)` returns:
  - `kind = MatchKind::CompleteSingle`,
  - `graphs = [VG-Z]` (only the complete graph; `VG-Z2` is dropped because its coverage is a strict subset),
  - `covered_by_match = [T1, T2, T3]`,
  - `residual_uncovered = []`.
- The matcher does **not** invoke [FT-048](FT-048)'s worker; that decision is the caller's, but the primitive itself remains side-effect-free.

## Notes

Validates the optimisation that drives [ADR-030](ADR-030)'s "match-or-generate is a deterministic precondition, not a worker decision". `CompleteSingle` is the simplest happy path.