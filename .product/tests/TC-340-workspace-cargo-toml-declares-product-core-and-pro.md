---
id: TC-340
title: Workspace Cargo.toml declares product-core and product-mcp at the pinned SHA
type: scenario
status: unimplemented
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/workspace-cargo-toml-shape.sh
runner-timeout: 30
observes:
- file
---

## Acceptance criteria

Verifies that [FT-136](FT-136) §Phase 1's Cargo wiring is intact. Catches an implementer regression where a future change drops one of the new deps, switches `rev` to `branch`/`tag` (defeats reproducibility), or fails to remove the deleted crates from `[workspace] members`.

### Conditions

- Root `Cargo.toml` `[workspace.dependencies]` table contains a key `product-core` with `git = "https://github.com/Hafeok/product-cli"` and `rev = "5fad7aa11ca8787ff74e87bb00e1cc0bdfb8b2c1"`.
- Root `Cargo.toml` `[workspace.dependencies]` table contains a key `product-mcp` with the same `git` and `rev`.
- Neither entry uses `branch = ...` or `tag = ...` (SHA-pin only).
- Root `Cargo.toml` `[workspace] members` array contains neither `"crates/product-cli"` nor `"crates/product-shim"`.
- `crates/decision-cli/Cargo.toml` `[dependencies]` declares both `product-core = { workspace = true }` and `product-mcp = { workspace = true }`.

### Exit codes

- `0` — all conditions hold.
- `1` — at least one condition fails. Script prints which condition failed and the offending content.

### Surface

`file` — assertion is against checked-in `Cargo.toml` files.
