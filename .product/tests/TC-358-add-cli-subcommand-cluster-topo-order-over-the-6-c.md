---
id: TC-358
title: 'add-cli-subcommand: Cluster::topo_order over the 6 cells is acyclic and deterministic'
type: exit-criteria
status: passing
validates:
  features:
  - FT-142
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --lib core::task_type::tests::cli_subcommand_topo_order
runner-timeout: 60
observes:
- exit-code
last-run: 2026-06-04T15:47:45.847784661+00:00
last-run-duration: 0.2s
---

## Description

Exit-criterion for the topo-order invariant of the `add-cli-subcommand` TaskType cluster. Validates that the FT-139 `Cluster::topo_order` helper, fed the six-cell declaration from `add-cli-subcommand`, returns a valid topological ordering that respects every `derived_from` edge and is deterministic across runs.

## Acceptance criteria

1. `Cluster::topo_order(&add_cli_subcommand_cells())` returns `Ok(order)` — no `PlanError::ClusterCycle`.
2. The returned `order` satisfies every `derived_from` edge:
   - `clap_args_module` precedes `handler_module`, `registration_wiring`, `mcp_tool_shim`, `integration_test`, and `help_doc_string`.
   - `handler_module` precedes `registration_wiring`, `mcp_tool_shim`, and `integration_test`.
3. `clap_args_module` is the first element (it is the only root — `derived_from: []`).
4. Re-running the topo sort twice on the same input returns byte-identical orders (determinism: ties are broken by cell-name lexicographic order, not by hash-map iteration order).
5. Including or excluding the optional `mcp_tool_shim` cell (per `surfaces_via_mcp` flag) produces an ordering that still satisfies every remaining `derived_from` edge.

## Runner

`cargo-test --package decision-cli --lib core::task_type::tests::cli_subcommand_topo_order` — exit 0 = pass; non-zero = fail.

## What this guards

Pinning topo-order determinism at the unit-test level is the cheapest signal that the cluster's `derived_from` graph is well-formed before it ships into the dispatcher. A non-deterministic order would make cluster runs non-reproducible (different cells dispatched in different orders against the same upstream bundles); a missing edge would surface as `clap_args_module` racing `handler_module` and emitting nonsense.