---
id: TC-354
title: add-artifact-type cluster topological order is acyclic and deterministic
type: exit-criteria
status: passing
validates:
  features:
  - FT-141
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib core::task_type::tests::artifact_type_topo_order
runner-timeout: 120
observes:
- exit-code
last-run: 2026-06-04T15:47:45.339341071+00:00
last-run-duration: 0.2s
---

## Context

Exit-criteria TC for [FT-141](FT-141) (TaskType `add-artifact-type`). Asserts the substrate's topological-sort contract from [FT-139](FT-139) generalizes correctly to the six-cell artifact-type cluster's `derived_from` graph.

## Setup

- Static registry from `crates/decision-cli/src/core/task_type/` populated with the `add-artifact-type` `TaskTypeDecl` (six cells: `rust_struct`, `shacl_shape`, `iri_module_consts`, `parser`, `emitter`, `round_trip_tests`).
- `derived_from` edges as declared in FT-141's §Outputs:
  - `rust_struct` → []
  - `shacl_shape` → [`rust_struct`]
  - `iri_module_consts` → [`rust_struct`]
  - `parser` → [`rust_struct`, `iri_module_consts`]
  - `emitter` → [`rust_struct`, `iri_module_consts`]
  - `round_trip_tests` → [`rust_struct`, `shacl_shape`, `parser`, `emitter`]

## Steps

1. Construct the `add-artifact-type` `TaskTypeDecl` from the registry constructor.
2. Call `Cluster::topo_order(&decl.cells)`.

## Expected outcome

- Result is `Ok(order)` (no cycle).
- For every edge `(a, b)` in the cluster's `derived_from` (where `a` derives from `b`), `b` appears before `a` in `order`.
- `rust_struct` is the first element; `round_trip_tests` is the last element.
- The result is deterministic across repeated invocations (same input → same output ordering). This is exit-criteria for the slice: if the topo order is non-deterministic, the cluster dispatch from FT-139 cannot reliably reproduce a cluster run.

## Pass / fail

- Pass: `cargo test --package decision-cli --lib core::task_type::tests::artifact_type_topo_order` exits 0.
- Fail: any assertion above does not hold; cargo-test exits non-zero.

## Why this is the exit-criteria TC

The cluster's `derived_from` graph IS the cluster contract — the substrate's topological soundness on this specific graph is the slice's structural completion gate. Without this, none of the scenario TCs are meaningful (a cyclic / non-deterministic order means the cluster dispatcher cannot honour the declared cell ordering, and the parser / emitter / round-trip-tests cells would be free to run before their upstream `rust_struct` + `iri_module_consts` sources land).