---
id: TC-256
title: DoR planner dispatches verify-graph-author when coverage is missing and returns Done once the covering VG is accepted
type: scenario
status: passing
validates:
  features:
  - FT-119
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::ft_119_drive_def_ready::dispatch_tests::tc_256
runner-timeout: 120
observes:
- exit-code
- graph
last-run: 2026-06-02T12:04:43.919578702+00:00
last-run-duration: 0.2s
---

## Claim

When DoR fails because no `dec:VerificationGraph` covers all of a feature's
TCs, the driver dispatches the verify-graph-author worker; once the resulting
VG is accepted out of `pending_review`, the next iteration's classification
flips to `Done` and the driver terminates with `DriveOutcome::Reached`.

This is the only worker-resolvable gap in the DoR classification table;
every other gap is `Stuck` because it needs a human.

## Scenarios

### Setup

Use a `MutableStubInspector` whose state evolves across calls to model:

- **Round 0**: `spec_complete=true, preflight=clean, deps_done=true,
  tcs_linked=true, tcs_ok=true, vgs_cover=false, vgs_accepted=false`.
- **Round 1**: same row but `vgs_cover=true, vgs_accepted=true` (the VGA
  dispatch is intercepted by a fake executor that flips the inspector's
  state to simulate worker success).

The fake executor records every `DispatchVerifyGraphAuthor { feature_id,
env_id }` it receives.

### Test

1. Call `drive::run` with `max_iter = 4`, goal `DefReady`, artifact
   `FT-T256`, bench `BNCH-002`.
2. Assert outcome `Ok(DriveOutcome::Reached { iterations: 1, history })`.
3. Assert `history.len() == 2`: round-0 is
   `DispatchVerifyGraphAuthor { feature_id: "FT-T256", env_id: "BNCH-002" }`,
   round-1 is `Done`.
4. Assert the fake executor was called exactly once.

### Pending-review case

Re-configure the inspector so round 1 ends with `vgs_cover=true` but
`vgs_accepted=false`. Assert the outcome is
`Err::Stuck { reason ~= /^VG pending_review:/ }` and the executor was called
exactly once.

### Boundary

- The verify-graph-author dispatch carries the same `env_id` for every
  iteration within a single drive. The planner reads it once from
  `PlanContext::default_bench` (or the `--bench` override). Switching mid-loop
  is forbidden.

## Notes

This TC is the integration backstop for the worker-resolvable arm of the
DoR table. Together with TC-254 (pure classifier) and TC-255 (Stuck reason
identity), it proves the driver wiring composes cleanly with the existing
FT-110 dispatch executor — no DoR-specific dispatch path needed.