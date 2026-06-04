---
id: TC-347
title: def-ready still dispatches verify-graph-author when no open implementer feedback exists
type: scenario
status: unimplemented
validates:
  features:
  - FT-138
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib features::ft_119_drive_def_ready::planner::tests::no_feedback_preserves_dispatch_verify_graph_author
runner-timeout: 60
observes:
- exit-code
---

## Acceptance criteria

Regression guard for [FT-138](FT-138) / [ADR-079](ADR-079): the new classifier row does not change behaviour when no open implementer feedback exists. The VG-missing → DispatchVerifyGraphAuthor path continues to fire for features that need a graph authored from scratch.

### Conditions

Unit test using a `StubInspector` returning:
- `feature_spec_completeness` → `Complete`
- `preflight_status_for_feature` → `Clean`
- `dependency_statuses_for_feature` → `[]`
- `tcs_linked_state_for_feature` → `AllReady`
- `has_open_implementer_feedback_for_feature` → `false` ← critical: no feedback open
- `covering_graph_state_for_feature` → `Missing`

Assert:
- `FeatureReadyPlanner::new(stub).classify("FT-T347", "BNCH-002")` returns `Action::DispatchVerifyGraphAuthor { feature_id: "FT-T347", bench_id: "BNCH-002" }`.
- The new feedback row did NOT short-circuit to Done — pre-FT-138 behaviour is preserved when the feedback signal is absent.

### Exit codes

- `0` — classifier returned `DispatchVerifyGraphAuthor`, preserving existing behaviour.
- `1` — classifier returned `Done` (the new row fired incorrectly) or anything else.

### Surface

`exit-code` — cargo-test against a stub inspector.
