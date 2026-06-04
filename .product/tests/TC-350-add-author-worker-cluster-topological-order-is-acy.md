---
id: TC-350
title: add-author-worker cluster topological order is acyclic and deterministic
type: exit-criteria
status: passing
validates:
  features:
  - FT-140
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib core::task_type::tests::author_worker_topo_order
runner-timeout: 120
last-run: 2026-06-04T15:47:44.739819852+00:00
last-run-duration: 0.2s
---

## Context

Exit-criteria TC for [FT-140](FT-140) (TaskType `add-author-worker`). Asserts the substrate's topological-sort contract from [FT-139](FT-139) generalizes correctly to the six-cell author cluster's `derived_from` graph.

## Setup

- Static registry from `crates/decision-cli/src/core/task_type/` populated with the `add-author-worker` TaskTypeDecl (six cells: `capability_binding`, `pydantic_io_models`, `system_prompt`, `agent_loop`, `fixtures_example_inputs`, `unit_tests`).
- `derived_from` edges as declared in FT-140's §Outputs:
  - `capability_binding` → []
  - `pydantic_io_models` → []
  - `system_prompt` → [`pydantic_io_models`]
  - `agent_loop` → [`pydantic_io_models`, `system_prompt`]
  - `fixtures_example_inputs` → [`pydantic_io_models`]
  - `unit_tests` → [`pydantic_io_models`, `fixtures_example_inputs`, `system_prompt`]

## Steps

1. Construct the `add-author-worker` `TaskTypeDecl` from the registry constructor.
2. Call `Cluster::topo_order(&decl.cells)`.

## Expected outcome

- Result is `Ok(order)` (no cycle).
- For every edge `(a, b)` in the cluster's `derived_from` (where `a` derives from `b`), `b` appears before `a` in `order`.
- The result is deterministic across repeated invocations (same input → same output ordering). This is exit-criteria for the slice: if the topo order is non-deterministic, the cluster dispatch from FT-139 cannot reliably reproduce a cluster run.

## Pass / fail

- Pass: `cargo test --package decision-cli --lib core::task_type::tests::author_worker_topo_order` exits 0.
- Fail: any assertion above does not hold; cargo-test exits non-zero.

## Why this is the exit-criteria TC

The cluster's `derived_from` graph IS the cluster contract — the substrate's topological soundness on this specific graph is the slice's structural completion gate. Without this, none of the scenario TCs are meaningful (a cyclic / non-deterministic order means the cluster dispatcher cannot honour the declared cell ordering).