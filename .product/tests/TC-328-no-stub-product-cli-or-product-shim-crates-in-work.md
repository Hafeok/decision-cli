---
id: TC-328
title: No stub product-cli or product-shim crates in workspace after FT-136
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/no-stub-product-crates.sh
runner-timeout: 30
observes:
- disk-state
---

## Acceptance criteria

Verifies that [FT-136](FT-136)'s deletion of `crates/product-cli/` and `crates/product-shim/` is complete and that the workspace `Cargo.toml` no longer references either crate.

### Conditions

- `crates/product-cli/` directory does not exist in the working tree.
- `crates/product-shim/` directory does not exist in the working tree.
- Root `Cargo.toml`'s `[workspace] members` array contains neither `"crates/product-cli"` nor `"crates/product-shim"`.
- Root `Cargo.toml`'s `[workspace.dependencies]` table contains no key named `product-cli` (the workspace now consumes `product-core` and `product-mcp` only).
- `crates/decision-cli/Cargo.toml`'s `[dependencies]` table contains no key named `product-cli`.

### Exit codes

- `0` — all conditions hold.
- `1` — any condition fails. The script prints which condition failed.

### Surface

`disk-state` — assertion is purely against checked-in files.
