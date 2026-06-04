---
id: TC-366
title: extend-role-catalog-seed cluster topological order is acyclic and deterministic including conditional cells
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-144
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib core::task_type::tests::role_catalog_seed_topo_order
runner-timeout: 120
observes:
- exit-code
---

## Context

Exit-criteria TC for [FT-144](FT-144) (TaskType `extend-role-catalog-seed`). Asserts the substrate's topological-sort contract from [FT-139](FT-139) generalizes correctly to the six-cell role-catalog-seed cluster's `derived_from` graph, including the two conditional cells (`shacl_shape_extension`, `role_struct_field_extension`) when both `requires_shacl=true` and `surfaces_on_role_struct=true` are declared.

## Setup

- Static registry from `crates/decision-cli/src/core/task_type/` populated with the `extend-role-catalog-seed` TaskTypeDecl (six cells: `iri_constants`, `seed_quad_function`, `init_pipeline_wiring`, `shacl_shape_extension`, `role_struct_field_extension`, `round_trip_tests`).
- `derived_from` edges as declared in FT-144's §Outputs:
  - `iri_constants` → []
  - `seed_quad_function` → [`iri_constants`]
  - `init_pipeline_wiring` → [`seed_quad_function`]
  - `shacl_shape_extension` → [`iri_constants`, `seed_quad_function`]  *(conditional on `requires_shacl=true`)*
  - `role_struct_field_extension` → [`iri_constants`]  *(conditional on `surfaces_on_role_struct=true`)*
  - `round_trip_tests` → [`seed_quad_function`, `init_pipeline_wiring`, `shacl_shape_extension`, `role_struct_field_extension`]  *(last two edges conditional)*

## Steps

1. Construct the `extend-role-catalog-seed` `TaskTypeDecl` from the registry constructor with cluster parameters `requires_shacl=true, surfaces_on_role_struct=true` (the maximal case — all six cells present).
2. Call `Cluster::topo_order(&decl.cells)`.
3. Repeat step 1 with `requires_shacl=false, surfaces_on_role_struct=false` (the minimal four-cell case) and call `Cluster::topo_order` again.

## Expected outcome

- For the maximal case: result is `Ok(order)` (no cycle); `order.len() == 6`; for every edge `(a, b)` in the cluster's `derived_from` (where `a` derives from `b`), `b` appears before `a` in `order`; specifically `iri_constants` precedes everything, `seed_quad_function` precedes `init_pipeline_wiring` / `shacl_shape_extension` / `round_trip_tests`, and `round_trip_tests` is last (or tied for last with no descendant).
- For the minimal case: result is `Ok(order)` (no cycle); `order.len() == 4`; the conditional cells are absent; `round_trip_tests` still appears last; `iri_constants → seed_quad_function → init_pipeline_wiring → round_trip_tests` is the only valid ordering for the linear chain.
- Determinism: both invocations produce the same output ordering across repeated calls (same input → same output). This is exit-criteria for the slice — if the topo order is non-deterministic, the cluster dispatch from FT-139 cannot reliably reproduce a cluster run for this TaskType.

## Pass / fail

- Pass: `cargo test --package decision-cli --lib core::task_type::tests::role_catalog_seed_topo_order` exits 0 (both maximal and minimal cases pass all assertions).
- Fail: any assertion above does not hold; cargo-test exits non-zero.

## Why this is the exit-criteria TC

The cluster's `derived_from` graph IS the cluster contract — the substrate's topological soundness on this specific six-cell graph (with conditional-cell branching) is the slice's structural completion gate. Without this, none of the scenario TCs (TC-367/368/369) are meaningful: a cyclic or non-deterministic order means the cluster dispatcher cannot honour the declared cell ordering, and the audit's six checks have nothing to assert against. The maximal+minimal pair specifically asserts that conditional-cell branching does not corrupt the topo invariant — a footgun FT-144 introduces relative to FT-139's purely-mandatory-cell prototype.