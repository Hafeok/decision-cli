---
id: TC-202
title: Per-feature drive runs under tokio time-out and yields Timeout outcome
type: scenario
status: unimplemented
validates:
  features:
  - FT-111
  adrs: []
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_202_per_feature_timeout_yields_timeout_outcome
runner-timeout: 30
---

## Description

PAT-003's per-item bounded execution is enforced via
`tokio::time::timeout`. When a per-feature drive runs longer
than the configured bound, the sweep records
`SweepOutcome::Timeout { after_secs }` and proceeds to the next
feature — the long-running future is dropped at its first await
point as part of `tokio::time::timeout`'s cancellation contract.

## Acceptance Criteria

Stub the per-feature driver as a future that sleeps for 5
seconds. Configure the sweep with `per_feature_timeout = 1s`
and a one-feature set `[FT-X]`. Assert:

1. The whole sweep completes in less than 2 seconds (proves the
   timeout interrupted the per-feature future, didn't wait it
   out).
2. The single row's outcome is `Timeout { after_secs: 1 }`.
3. The tally has `timeout = 1`, all other buckets `0`.

Uses `tokio::test` with `start_paused = true` for deterministic
time advancement; no real wall-clock waits.
