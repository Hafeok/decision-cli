---
id: TC-362
title: extend-planner-classifier cluster topological order is acyclic and deterministic
type: exit-criteria
status: passing
validates:
  features:
  - FT-143
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib core::task_type::tests::planner_classifier_topo_order
runner-timeout: 120
observes:
- exit-code
last-run: 2026-06-04T15:47:46.291419308+00:00
last-run-duration: 0.2s
---

## Context

Exit-criteria TC for [FT-143](FT-143) (TaskType `extend-planner-classifier`). Asserts the substrate's topological-sort contract from [FT-139](FT-139) generalizes correctly to the six-cell planner-classifier cluster's `derived_from` graph.

## Setup

- Static registry from `crates/decision-cli/src/core/task_type/` populated with the `extend-planner-classifier` `TaskTypeDecl` (six cells: `inspector_trait_method`, `inspector_default_impl`, `inspector_production_impl`, `classifier_row`, `state_hash_update`, `unit_tests`).
- `derived_from` edges as declared in FT-143's §Outputs:
  - `inspector_trait_method` → []
  - `inspector_default_impl` → [`inspector_trait_method`]
  - `inspector_production_impl` → [`inspector_trait_method`]
  - `classifier_row` → [`inspector_trait_method`, `inspector_production_impl`]
  - `state_hash_update` → [`inspector_trait_method`, `classifier_row`]
  - `unit_tests` → [`inspector_trait_method`, `classifier_row`, `state_hash_update`]

## Steps

1. Construct the `extend-planner-classifier` `TaskTypeDecl` from the registry constructor.
2. Call `Cluster::topo_order(&decl.cells)`.

## Expected outcome

- Result is `Ok(order)` (no cycle).
- For every edge `(a, b)` in the cluster's `derived_from` (where `a` derives from `b`), `b` appears before `a` in `order`.
- The result is deterministic across repeated invocations (same input → same output ordering). This is exit-criteria for the slice: if the topo order is non-deterministic, the cluster dispatch from FT-139 cannot reliably reproduce a cluster run.
- Specifically: `inspector_trait_method` is index 0; `unit_tests` is last; `classifier_row` precedes both `state_hash_update` and `unit_tests`; `state_hash_update` precedes `unit_tests`; `inspector_production_impl` precedes `classifier_row`; `inspector_default_impl` may appear anywhere after `inspector_trait_method`.

## Pass / fail

- Pass: `cargo test --package decision-cli --lib core::task_type::tests::planner_classifier_topo_order` exits 0.
- Fail: any assertion above does not hold; cargo-test exits non-zero.

## Why this is the exit-criteria TC

The cluster's `derived_from` graph IS the cluster contract — the substrate's topological soundness on this specific graph is the slice's structural completion gate. Without this, none of the audit-scenario TCs are meaningful (a cyclic / non-deterministic order means the cluster dispatcher cannot honour the declared cell ordering, and the audit's positional-comment check in TC-363 would compare against a non-canonical layout).