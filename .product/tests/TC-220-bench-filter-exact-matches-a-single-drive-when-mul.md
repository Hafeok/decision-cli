---
id: TC-220
title: Bench filter exact-matches a single drive when multiple drives exist
type: scenario
status: unimplemented
validates:
  features:
  - FT-113
  adrs: []
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_220_bench_filter_isolates_single_drive
runner-timeout: 30
---

## Description

A feature can have multiple drive runs across different
benches (BNCH-001 for clean-slate verification, BNCH-002 for
mid-cycle, etc.). The default render shows the most recent
across all benches; `--bench BNCH-NNN` should hide every
round that ran on a different bench.

## Acceptance Criteria

Cargo test:

1. Build a store with two drives for FT-X:
   - Drive A on BNCH-001: 2 rounds, started at t=0.
   - Drive B on BNCH-002: 3 rounds, started at t=600.
2. Call `reader.rounds_for_feature("FT-X", None)`. Assert it
   returns Drive B's 3 rounds (most recent by default).
3. Call `reader.rounds_for_feature("FT-X", Some("BNCH-001"))`.
   Assert it returns exactly Drive A's 2 rounds; no Drive B
   round appears.
4. Call with a bench that has no drives: `Some("BNCH-099")`.
   Assert it returns `Vec::new()` (renderer then shows the
   empty-state from TC-216).
