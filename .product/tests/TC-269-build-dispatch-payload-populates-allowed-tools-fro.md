---
id: TC-269
title: build_dispatch_payload populates allowed_tools from role catalog lookup
type: scenario
status: unimplemented
validates:
  features:
  - FT-122
  adrs:
  - ADR-008
  - ADR-070
phase: 4
observes:
- graph
runner: cargo-test
runner-args: tc_269_build_dispatch_payload_carries_allowed_tools
runner-timeout: 30
---

## Description

FT-122 threads the role-catalog tool surface into the dispatch payload. This TC asserts the integration point: given a store seeded by FT-121, `build_dispatch_payload()` returns a `DispatchPayloadJson` whose `allowed_tools` field equals the implementer role's seeded list.

This is the unit-level integration test. TC-270 covers the wire shape; TC-271 covers fail-closed behaviour on empty.

## Acceptance Criteria

Given an in-memory `Store` seeded by `core::role_catalog::seeds::seed_default_roles(&store, &graph_name)`:

- Call `features::implement::lifecycle::build_dispatch_payload(&ctx, &args)` against a synthetic implementer dispatch context.
- The returned `DispatchPayloadJson.allowed_tools` field equals `vec!["read_file", "write_file", "run_build", "run_lint", "run_tests"]` (order-insensitive).
- The remaining payload fields (`endpoint`, `model_id`, etc.) match the pre-FT-122 baseline — the change is strictly additive.

Lives at `crates/decision-cli/src/features/implement/lifecycle.rs::tests::tc_269_build_dispatch_payload_carries_allowed_tools`. Test setup reuses the existing implement-feature test fixtures.
