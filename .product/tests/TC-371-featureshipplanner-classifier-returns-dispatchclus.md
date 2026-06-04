---
id: TC-371
title: FeatureShipPlanner classifier returns DispatchCluster for task_type front-matter; falls through to DispatchImplementer otherwise
type: scenario
status: unimplemented
validates:
  features:
  - FT-139
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib features::drive::planners::feature_ship::tests::classifier_returns_dispatch_cluster_for_task_type_frontmatter
runner-timeout: 60
observes:
- exit-code
---

## Acceptance criteria

Verifies that [FT-139](FT-139)'s classifier branch reads `task_type:` from the feature_spec front-matter and returns `Action::DispatchCluster` when it names a registered TaskType, falling through to `Action::DispatchImplementer` (the broad-worker path) otherwise. Locks in [ADR-080](ADR-080)'s escape-hatch principle.

### Conditions

Unit test in `crates/decision-cli/src/features/drive/planners/feature_ship.rs` or sibling tests file.

- **Positive case** — fixture feature_spec with `task_type: add-judge-worker` in front-matter; classifier returns `Action::DispatchCluster { task_type_name: "add-judge-worker", feature_id }`.
- **Fallthrough case (absent)** — fixture without any `task_type:` field; classifier returns `Action::DispatchImplementer { feature_id, .. }` (existing pre-FT-139 behaviour preserved).
- **Fallthrough case (unknown)** — fixture with `task_type: not-a-real-type`; classifier returns `Action::DispatchImplementer` (low-confidence → broad worker per ADR-080's escape hatch, NOT a `PlanError`).
- Precedence: classifier branch fires BEFORE the existing `DispatchImplementer` action and AFTER the existing TC/VG checks.

### Exit codes

- `0` — all three branches produce the expected `Action` variant.
- `1` — any branch returns the wrong variant or a `PlanError`.

### Surface

`exit-code` — cargo-test against a stub feature loader.
