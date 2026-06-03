---
id: TC-331
title: product-core's Cargo.toml has no dependency on decision-cli or oxi-events
type: scenario
status: passing
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/product-core-sdp-boundary.sh
runner-timeout: 90
observes:
- file
last-run: 2026-06-03T12:09:07.168277302+00:00
last-run-duration: 0.2s
---

## Acceptance criteria

Verifies the SDP-boundary invariant carried forward from [ADR-009](ADR-009) and [ADR-067](ADR-067) into [ADR-077](ADR-077): `product-core` (the crate this workspace depends on) has no dependency on `decision-cli` or `oxi-events`.

With the Cargo-dep transport adopted in ADR-077, this property is mostly enforced by the upstream factoring itself — `product-core` is in a separate repository with no such dep. This TC is the explicit, automated guard against a future upstream change accidentally introducing the cycle.

### Conditions

- Run `cargo fetch --locked` at the workspace root (idempotent if already fetched).
- Locate the fetched `product-core` Cargo.toml. It lives under `${CARGO_HOME:-$HOME/.cargo}/git/checkouts/product-cli-*/<sha>/product-core/Cargo.toml` for git deps, or `${CARGO_HOME:-$HOME/.cargo}/registry/src/<index>/product-core-*/Cargo.toml` if migrated to a registry source.
- `grep -E '^(decision-cli|oxi-events)\s*=' <Cargo.toml>` returns no match in any of the dependency tables (`[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, target-specific variants).

### Exit codes

- `0` — `product-core`'s Cargo.toml is clean.
- `1` — a forbidden dependency was found. The script prints the offending line and the file path.
- `2` — `cargo fetch` failed or the Cargo.toml could not be located (unrunnable, distinct from a SDP violation).

### Surface

`file` — assertion is against the fetched-on-disk Cargo.toml content.