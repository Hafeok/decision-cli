---
id: TC-258
title: 'DoR planner inherits PAT-002 cycle detection: a VGA re-author oscillation yields Stuck with a period-N reason'
type: scenario
status: passing
validates:
  features:
  - FT-119
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::ft_119_drive_def_ready::planner::tests::tc_258
runner-timeout: 120
observes:
- exit-code
- stdout
last-run: 2026-06-02T12:26:35.495294390+00:00
last-run-duration: 0.2s
---

## Claim

`FeatureReadyPlanner` inherits PAT-002's state-hash cycle detection: when the
worker addresses a defect but the planner observation returns to a previously
seen state (a VGA re-authoring oscillation), the loop terminates with
`Stuck { reason }` whose text identifies the cycle period.

## Scenarios

### Setup

Use a `MutableStubInspector` that cycles through three observation states
per the VGA-oscillation pattern:

- **State α**: `vgs_cover=false` → planner dispatches VGA.
- **State β**: `vgs_cover=true, vgs_accepted=false` → planner emits
  `Stuck { reason ~= /VG pending_review/ }`.
- **State γ**: `vgs_cover=false` (the human superseded the pending VG,
  putting coverage back to none) → planner dispatches VGA again.

A fake executor advances the inspector through α → β → γ → β → γ → … on
each `DispatchVerifyGraphAuthor`.

### Test

1. Call `drive::run` with `max_iter = 12`, goal `DefReady`, artifact
   `FT-T258`.
2. Assert outcome `Err::Stuck { reason }`.
3. Assert `reason.contains("state-hash cycle")`.
4. Assert `reason.contains("period")` and the printed period is `2` or `3`
   (depending on how the oscillation maps onto recorded hashes).
5. Assert the loop terminated strictly before `max_iter` (i.e. the cycle
   detector fired, not the iteration cap).
6. Assert `history.len()` ≤ 8 (PAT-002's ring buffer is 8 slots; a period-2
   or period-3 cycle is detected within ≤ STATE_HASH_BUFFER_LEN + period
   iterations).

### Pair-wise reason takes precedence

A second scenario configures the inspector so the pairwise no-progress
detector fires first (e.g. two consecutive VGA dispatches against an
unchanged inspector state). Assert the `Stuck` reason is the pairwise reason
(`"verify-graph-author dispatch did not change state"` or equivalent) and
NOT the generic cycle reason. This is PAT-002's "pairwise gets the
diagnostic" rule.

### Cross-feature isolation

Run `drive::run` twice in the same process, first for `FT-T258a`, then for
`FT-T258b`, sharing one planner instance. The ring buffer must reset on the
feature-id change. Assert the second drive does not false-positive a cycle
inherited from the first.

## Notes

This TC is the cycle-detection backstop. Without it, a regression that
fails to feed the DoR-relevant dimensions into `state_hash_for_feature`
(e.g. only hashing the verdict, missing the `vgs_cover` bit) would still
pass TC-254 (pure classification) but would let the VGA-oscillation loop
run to `max_iter` instead of failing fast with a graph-theoretic reason.