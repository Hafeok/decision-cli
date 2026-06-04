---
id: TC-268
title: Legacy stores without dec:roleTool quads return empty allowed_tools and do not panic
type: scenario
status: passing
validates:
  features:
  - FT-121
  adrs:
  - ADR-042
  - ADR-070
phase: 4
observes:
- graph
runner: cargo-test
runner-args: tc_268_legacy_store_returns_empty_allowed_tools
runner-timeout: 30
last-run: 2026-06-04T09:05:26.005362752+00:00
last-run-duration: 0.5s
---

## Description

Stores authored before FT-121 lands have no `dec:roleTool` quads. Per [ADR-042](ADR-042) (grandfathered-with-backfill), the lookup path must not panic on these stores — it returns `allowed_tools: vec![]`, and the worker fail-closes at dispatch time per [ADR-069](ADR-069). This TC asserts the legacy-compat path.

The SHACL pass is allowed to surface a *warning* on legacy stores (advisory); the *lookup* path is required to be operational.

## Acceptance Criteria

Construct an in-memory `Store` containing a `dec:Role` instance with `dec:roleId`, `dec:roleOutputType`, `dec:roleInputType`, `dec:roleModelBinding` — but **no** `dec:roleTool` quads (simulating a pre-FT-121 seed).

- `role_catalog::lookup(&store, "implementer")?` returns `Some(Role { .. })` — the lookup succeeds.
- The returned `Role.allowed_tools` is `vec![]` (empty Vec, not `None`, not a panic).
- All other fields on `Role` (`input_types`, `output_type`, `model_binding`) populate normally.
- Calling `lookup` twice on the same store returns identical values (no hidden mutation).

Lives at `crates/decision-cli/src/core/role_catalog/tests.rs::tc_268_legacy_store_returns_empty_allowed_tools`. Reuses the legacy-store fixture pattern already used by the existing `collect_input_types` tests in that module.