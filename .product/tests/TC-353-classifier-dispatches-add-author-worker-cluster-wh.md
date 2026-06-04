---
id: TC-353
title: classifier dispatches add-author-worker cluster when task_type front-matter declares it
type: scenario
status: unimplemented
validates:
  features:
  - FT-140
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib features::ft_139_cluster_dispatch::tests::author_worker_classifier_branch
runner-timeout: 120
---

## Context

Classifier-branch TC for [FT-140](FT-140). Asserts the `FeatureShipPlanner::classify` extension from [FT-139](FT-139) generalizes correctly to the new `add-author-worker` TaskType registry entry — the classifier returns the cluster-dispatch action when a feature_spec declares `task_type: add-author-worker`, and preserves the broad-worker fallback when the front-matter is absent or unknown.

## Setup

- The FT-139 substrate is in place: classifier branch added to `features/drive/planners/feature_ship.rs`, `Action::DispatchCluster { task_type_name, feature_id }` exists on the action enum, `cluster_dispatch::run` is wired.
- The static TaskType registry contains the `add-author-worker` `TaskTypeDecl` registered by this slice.
- Three test fixture feature_specs constructed in a tempdir:
  - `FT-fixture-author/` with front-matter `task_type: add-author-worker` and otherwise minimal valid feature_spec front-matter.
  - `FT-fixture-implementer/` with NO `task_type:` field.
  - `FT-fixture-unknown/` with front-matter `task_type: not-a-real-task-type`.

## Steps

1. Construct a `FeatureShipPlanner` instance bound to the tempdir's product-cli root.
2. Call `classify` against each of the three fixtures.

## Expected outcome

- For `FT-fixture-author`: result is `Action::DispatchCluster { task_type_name: "add-author-worker", feature_id: "FT-fixture-author" }`.
- For `FT-fixture-implementer`: result is `Action::DispatchImplementer { .. }` (the broad-worker fallback). This is the ADR-080 escape-hatch path and MUST remain non-optional per FT-140's invariants.
- For `FT-fixture-unknown`: result is also `Action::DispatchImplementer { .. }` — unknown task types fall through to the broad worker (low-confidence path per ADR-080 §Decision §2).

## Pass / fail

- Pass: `cargo test --package decision-cli --lib features::ft_139_cluster_dispatch::tests::author_worker_classifier_branch` exits 0.
- Fail: any classification disagrees with the expected mapping above — e.g. the unknown task type dispatches a cluster (broad-worker escape hatch broken), or the declared author task type falls through to the implementer (registry entry not wired).

## Why this matters

The classifier branch is the integration seam between FT-140 (this TaskType declaration) and FT-139 (the substrate + dispatcher). If the registry entry is correctly populated but the classifier does not pick it up, the cluster never executes. If the classifier picks it up but the unknown-task fallback is broken, the ADR-080 escape hatch — explicitly load-bearing per the SDLC doc — is silently lost. Both halves must hold.
