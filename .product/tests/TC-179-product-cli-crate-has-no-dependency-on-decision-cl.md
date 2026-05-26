---
id: TC-179
title: product-cli crate has no dependency on decision-cli or oxi-events (SDP fitness check)
type: scenario
status: unimplemented
validates:
  features:
  - FT-105
  adrs: []
phase: 1
---

## Claim

`crates/product-cli/Cargo.toml` declares **no dependency** on `crates/decision-cli/` or `crates/oxi-events/`. The SDP direction `decision-cli → product-cli → (nothing in this workspace)` is structurally preserved by the absorption. Asserted at CI time so any future regression is caught immediately.

## Scenarios

### Setup

- Post-FT-105 absorbed workspace.

### Scenario A — Cargo.toml structural check

Parse `crates/product-cli/Cargo.toml` (e.g. via `cargo metadata --no-deps --format-version 1` filtered to the product-cli package). Assertions:

- `dependencies`, `dev-dependencies`, and `build-dependencies` sections contain **no entry** named:
  - `decision-cli`
  - `decision_cli` (snake_case form, in case Cargo normalises differently)
  - `oxi-events`
  - `oxi_events`
- Workspace-relative path dependencies (`path = "../decision-cli"` etc.) are also rejected.
- The check considers every Cargo target table (`[lib]`, `[[bin]]`, `[[test]]`, `[[bench]]`).

### Scenario B — Source-grep secondary assertion

Grep `crates/product-cli/src/**/*.rs` for `use decision_cli` and `use oxi_events`. Assertions:

- Zero hits. If any appear, even via re-export, the SDP boundary has been violated at the source level even if Cargo.toml is clean.
- The check accounts for fully-qualified paths too: `decision_cli::` and `oxi_events::` patterns.

### Scenario C — cargo metadata transitive check

Run `cargo metadata --no-deps --format-version 1 | jq '.packages[] | select(.name == "product-cli") | .dependencies[].name'`. Assertions:

- The list does not contain `decision-cli` or `oxi-events`.
- The list MAY contain external crates (oxigraph, serde, anyhow, clap, etc.) — those are not policed by this TC.

### Scenario D — Build product-cli in isolation

Run `cargo build -p product-cli`. Assertions:

- Exit code: 0.
- The build does NOT compile `decision-cli` or `oxi-events`. Verified by inspecting `cargo build -p product-cli --message-format=json` and asserting that the only crates compiled are `product-cli` itself and its external transitive deps.
- This is the strongest assertion: if decision-cli were a hidden dep, building product-cli in isolation would fail or pull it in.

### Scenario E — Reverse direction allowed

Sanity check: `crates/decision-cli/Cargo.toml` DOES list `product-cli = { path = "../product-cli" }`. This asserts the absorption actually wired the dependency in the intended direction (catches the case where the absorption forgot to add the dependency at all, leaving `dec product *` non-functional).

### Scenario F — CI gating

The TC's runner is registered in CI as a blocking check on every PR. A PR that adds `decision-cli` to product-cli's deps cannot merge until the dep is removed and the violation explained in the PR description (or, in the rare case the dep is legitimate, the SDP rule itself is amended via a new ADR).

## Runner

`bash tests/scripts/tc-179-sdp-product-cli.sh`. The script:

1. Runs `cargo metadata --no-deps --format-version 1` and pipes through `jq` for the structural checks.
2. Runs `grep -r 'use decision_cli\|use oxi_events\|decision_cli::\|oxi_events::' crates/product-cli/src/` and asserts no hits.
3. Runs `cargo build -p product-cli` and inspects `--message-format=json` output.
4. Exits 0 if all pass; exits 1 with a diagnostic naming the violating file or Cargo.toml line if any fail.

## Non-goals

- Asserting SDP boundaries between other crate pairs (each pair gets its own TC if it matters; this TC is product-cli-specific because the absorption introduced the new boundary).
- Detecting subtler coupling forms — shared file paths, shared environment variables, IPC — which can re-introduce coupling without showing up in Cargo.toml or `use` statements. Those are deeper concerns; this TC covers the structural minimum.
- Auto-fixing a violation — operators handle the fix per the diagnostic.
