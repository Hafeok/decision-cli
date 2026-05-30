---
id: TC-222
title: Discovery walks up from workdir to find .product/ and errors clearly when none found
type: scenario
status: passing
validates:
  features:
  - FT-114
  adrs: []
observes:
- stdout
- stderr
- exit-code
phase: 4
runner: cargo-test
runner-args: tc_222_discovery_walks_up_or_errors
runner-timeout: 30
last-run: 2026-05-30T16:41:58.446771378+00:00
last-run-duration: 0.5s
---

## Description

`dec init` (no args) must find the operator's `.product/`
graph whether they ran it from the repo root or a nested
subdirectory. Without walk-up, operators get a cryptic
"no .product/" error from a sub-shell and bounce off the tool.
With walk-up, the discovery matches product-cli's existing
convention so muscle memory transfers.

## Acceptance Criteria

Cargo test using temp directories:

1. **Walk-up success.** Compose a temp repo:
   `<temp>/some-repo/.product/product.toml` exists; the test
   sets `--workdir <temp>/some-repo/crates/inner/`. Assert the
   discovery routine returns the path
   `<temp>/some-repo/.product/`.
2. **Workdir-direct success.** Same repo; set `--workdir
   <temp>/some-repo/`. Assert the discovery routine returns
   the same path.
3. **Not-found error.** Set `--workdir <temp>/empty-dir/`
   (no `.product/` anywhere up the chain). Assert the
   discovery routine returns
   `Err(InitError::NoProductGraph { searched_from })`.
4. **Error message contains the searched path** and the
   remediation hint `"product feature new"`.

Pure-function test against the discovery routine; no
filesystem mocks beyond the temp directory.