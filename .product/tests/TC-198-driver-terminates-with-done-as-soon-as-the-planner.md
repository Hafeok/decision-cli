---
id: TC-198
title: Driver terminates with Done as soon as the planner reports the goal is reached
type: exit-criteria
status: passing
validates:
  features:
  - FT-110
  adrs: []
phase: 3
runner: cargo-test
runner-args: tc_198_driver_done_short_circuits
runner-timeout: 60
last-run: 2026-05-27T12:28:34.594881822+00:00
last-run-duration: 0.7s
---

## Claim

`drive::run` terminates with `DriveOutcome::Reached` on the *first* iteration that the planner returns `Action::Done`. No further dispatches are attempted. An already-satisfied goal (planner returns `Done` immediately on the first call) yields `iterations == 0`.

## Scenarios

### Test A — already-satisfied goal

A test planner returns `Action::Done` on the first call. `drive::run` with any `max_iter ≥ 1` returns:

1. `Ok(DriveOutcome::Reached { iterations: 0, history })`.
2. `history` has exactly one entry — the `Done` action.
3. The dispatch executor was never called.

### Test B — Done after one fix

A test planner returns `Action::DispatchImplementer` on the first call, then `Action::Done` on the second. `drive::run` with `max_iter = 5` returns:

1. `Ok(DriveOutcome::Reached { iterations: 1, history })`.
2. `history` has two entries (Dispatch then Done).
3. The dispatch executor was called exactly once.

### Boundary

- A planner that oscillates (returns `DispatchImplementer` forever despite the executor reporting success) hits the `max_iter` cap and surfaces as `Err::MaxIterations`. This is the natural complement to TC-197's stuck-detection path.