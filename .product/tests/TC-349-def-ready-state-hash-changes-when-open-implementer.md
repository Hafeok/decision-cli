---
id: TC-349
title: def-ready state hash changes when open-implementer-feedback signal flips
type: scenario
status: passing
validates:
  features:
  - FT-138
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib features::ft_119_drive_def_ready::planner::tests::state_hash_includes_open_implementer_feedback_signal
runner-timeout: 60
observes:
- exit-code
last-run: 2026-06-04T08:36:34.025280866+00:00
last-run-duration: 0.2s
---

## Acceptance criteria

Locks in [FT-138](FT-138) §Phase 3's requirement that `classify_and_hash` folds the `has_open_implementer_feedback_for_feature` signal into the state hash. Without this property the cycle detector (PAT-002 ring buffer + pairwise `last_seen` snapshot) would false-positive when a defect transitions from `produced` → `addressed` between iterations: same inputs to the older dimensions → same hash → "no progress" → spurious `Stuck`. This TC is the silent-regression guard — an implementer who only adds the classifier row but forgets the hash update would pass TC-345/346/347 and only break things in live drives.

### Conditions

Unit test in `crates/decision-cli/src/features/ft_119_drive_def_ready/planner.rs::tests`.

Construct two `StubInspector` instances differing **only** in `has_open_implementer_feedback_for_feature`'s return value. Every other dimension returns identical results:
- `feature_spec_completeness` → `Complete`
- `preflight_status_for_feature` → `Clean`
- `dependency_statuses_for_feature` → `[]`
- `tcs_linked_state_for_feature` → `AllReady`
- `covering_graph_state_for_feature` → `Missing`
- `aggregate_verdict_for_feature` → (same value)

Compute the state hash for both via `FeatureReadyPlanner::classify_and_hash` (or whichever public/`pub(super)` API exposes the hash to tests):
- `let (_, hash_with_feedback)    = planner_a.classify_and_hash("FT-T349", "BNCH-002")?;` with stub returning `true`.
- `let (_, hash_without_feedback) = planner_b.classify_and_hash("FT-T349", "BNCH-002")?;` with stub returning `false`.

Assert:
- `hash_with_feedback != hash_without_feedback`.
- Both calls succeed (no `PlanError`).

### Why this matters

The cycle detector uses the hash as the "this iteration's observable state" key. If feedback flipping from `produced` → `addressed` (or vice versa) leaves the hash unchanged, two consecutive iterations look identical to the detector even when meaningful state has changed. The result: `dec drive ship` reporting `Stuck "dispatch:implementer did not change state"` immediately after the implementer's CodeChange addressed the only outstanding defect.

### Exit codes

- `0` — the two hashes differ; the boolean signal is in the hash.
- `1` — hashes match (regression: the signal isn't folded in) or either call returned `PlanError`.

### Surface

`exit-code` — cargo-test against two stub inspectors; pure computation, no I/O.