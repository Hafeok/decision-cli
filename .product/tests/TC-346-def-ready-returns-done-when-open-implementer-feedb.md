---
id: TC-346
title: def-ready returns Done when open implementer feedback exists and TCs are ready
type: scenario
status: passing
validates:
  features:
  - FT-138
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib features::ft_119_drive_def_ready::planner::tests::open_implementer_feedback_returns_done_before_vg_missing
runner-timeout: 60
observes:
- exit-code
last-run: 2026-06-04T08:36:34.025280866+00:00
last-run-duration: 0.1s
---

## Acceptance criteria

Positive case for [FT-138](FT-138) / [ADR-079](ADR-079): when all earlier classifier rows pass and open implementer feedback exists, the planner returns `Action::Done` **before** considering the VG-missing branch.

### Conditions

Unit test using a `StubInspector` returning:
- `feature_spec_completeness` → `Complete`
- `preflight_status_for_feature` → `Clean`
- `dependency_statuses_for_feature` → `[]`
- `tcs_linked_state_for_feature` → `AllReady`
- `has_open_implementer_feedback_for_feature` → `true`
- `covering_graph_state_for_feature` → `Missing` (deliberately set so the VG-missing branch *would* fire without the new row)

Assert:
- `FeatureReadyPlanner::new(stub).classify("FT-T346", "BNCH-002")` returns `Action::Done`.
- The classifier did **not** return `Action::DispatchVerifyGraphAuthor` (witness for ADR-079 §Decision precedence).

### Exit codes

- `0` — classifier returned `Done` per ADR-079.
- `1` — anything else (DispatchVerifyGraphAuthor, Stuck, etc.).

### Surface

`exit-code` — cargo-test against a stub inspector.