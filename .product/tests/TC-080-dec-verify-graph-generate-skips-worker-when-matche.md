---
id: TC-080
title: dec verify graph generate skips worker when matcher reports complete match
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: tc_080_dec_verify_graph_generate_skips_worker_when_matche
runner-timeout: 120
last-run: 2026-05-21T19:20:28.691484988+00:00
last-run-duration: 0.4s
---

## Premise

`dec verify graph generate FT-O --environment ENV-1` is invoked in a store that already contains a graph `VG-EX` covering all of `FT-O`'s TCs in `ENV-1`.

## Acceptance Criteria

- The matcher returns `MatchKind::CompleteSingle` for this query.
- The verify-graph-author worker subprocess is **not** spawned (verified via a process-spawn hook or invocation counter).
- The handler returns `GraphProposal::Match { graph_id: "VG-EX" }`.
- CLI prints "VG-EX already covers FT-O in ENV-1; no new graph needed" (or equivalent).
- Exit code is 0.
- No new `.ttl` is written.

## Notes

Validates the optimisation: the worker only runs when generation is actually needed. This protects Claude budget and keeps determinism on the happy path.