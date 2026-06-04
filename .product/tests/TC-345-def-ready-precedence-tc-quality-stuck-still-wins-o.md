---
id: TC-345
title: 'def-ready precedence: TC quality stuck still wins over open implementer feedback'
type: exit-criteria
status: passing
validates:
  features:
  - FT-138
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib features::ft_119_drive_def_ready::planner::tests::precedence_tc_quality_wins_over_open_implementer_feedback
runner-timeout: 60
observes:
- exit-code
last-run: 2026-06-04T08:35:21.140440710+00:00
last-run-duration: 0.2s
---

## Acceptance criteria

Verifies that [FT-138](FT-138)'s new "open implementer feedback → Done" classifier row is positioned **below** the TC-quality check in the precedence ladder. A TC-quality stuck condition wins regardless of whether open implementer feedback exists.

### Conditions

Unit test in `crates/decision-cli/src/features/ft_119_drive_def_ready/planner.rs` (or a sibling tests file).

Construct a `StubInspector` returning:
- `feature_spec_completeness` → `Complete`
- `preflight_status_for_feature` → `Clean`
- `dependency_statuses_for_feature` → `[]`
- `tcs_linked_state_for_feature` → `SomeUnready { problem_tc: "TC-999", problem: "runner missing" }`
- `has_open_implementer_feedback_for_feature` → `true`
- `covering_graph_state_for_feature` → `Missing`

Assert:
- `FeatureReadyPlanner::new(stub).classify("FT-T345", "BNCH-002")` returns `Action::Stuck { reason }` where `reason.contains("TC quality")` and `reason.contains("TC-999")`.
- The reason does NOT contain `"feedback"` (the lower-precedence row did not fire).

### Exit codes

- `0` — classifier returned the TC-quality Stuck and the new feedback row did not fire.
- `1` — classifier returned anything else.

### Surface

`exit-code` — cargo-test runs the classifier against a stub inspector; no I/O.