---
id: TC-266
title: role_catalog::lookup returns seeded allowed_tools for implementer role
type: scenario
status: passing
validates:
  features:
  - FT-121
  adrs:
  - ADR-070
phase: 4
observes:
- graph
runner: cargo-test
runner-args: tc_266_role_catalog_lookup_returns_seeded_allowed_tools
runner-timeout: 30
last-run: 2026-06-04T09:02:32.005072667+00:00
last-run-duration: 150.0s
---

## Description

[ADR-070](ADR-070) makes the role catalog the source of truth for the per-role tool surface. FT-121 implements the seed, the predicate, and the lookup. This TC asserts the end-to-end happy path: after a fresh seed, `role_catalog::lookup(&store, "implementer")` returns the canonical five-tool list, and the verifier role returns the four-tool subset.

## Acceptance Criteria

Given an oxigraph `Store` seeded by `core::role_catalog::seeds::seed_default_roles(&store, &graph_name)`:

- `role_catalog::lookup(&store, "implementer")?.unwrap().allowed_tools` equals `vec!["read_file", "write_file", "run_build", "run_lint", "run_tests"]` (order-insensitive comparison — the seed declares insertion order but SPARQL `SELECT DISTINCT` does not guarantee return order).
- `role_catalog::lookup(&store, "verifier")?.unwrap().allowed_tools` equals `vec!["read_file", "run_build", "run_lint", "run_tests"]` (same order-insensitive comparison).
- Both roles' `allowed_tools` fields are non-empty (`.len() >= 1`).
- The `read_file` literal appears under both roles' tool lists (sanity check the seeding covers shared tools).

The test lives at `crates/decision-cli/src/core/role_catalog/tests.rs::tc_266_role_catalog_lookup_returns_seeded_allowed_tools`. It uses the same in-memory `Store` setup the existing role-catalog tests use; no fixtures or test data files.