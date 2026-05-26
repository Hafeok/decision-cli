---
id: FT-105
title: 'decision-cli: Absorb product-cli into the workspace as crates/product-cli/ with direct Rust API integration'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-067
- ADR-009
- ADR-029
- ADR-016
tests:
- TC-176
- TC-177
- TC-178
- TC-179
domains: []
domains-acknowledged: {}
---

## Description

The implementation slice for [ADR-067](ADR-067)'s decision to absorb product-cli into the decision-cli Cargo workspace. The slice is mechanical (git subtree merge + Cargo wiring + CLI re-export + MCP combination + deprecation shim), bounded, and reversible: at any point the merged repo can produce a snapshot equivalent to a pre-absorption product-cli + decision-cli pair, so the migration is safe to roll forward in stages.

The slice deliberately **does not** refactor either codebase to share internals or merge their on-disk storage layouts. ADR-067 mandates that `.product/` and `.dec/` remain distinct on disk; the named-graphs strategy for the oxigraph projection is a *capability* added in this slice (a single store backing two named graphs) but the on-disk layout is preserved so no migration of existing checkouts is required.

After this slice lands, every cross-repo feature spec authored to date ([FT-076](FT-076), [FT-104](FT-104), and any other "product-cli surface" features) becomes implementable in a single intra-repo PR. CLAUDE.md's slice-3+ vocabulary (`dec product <subcommand>`) becomes the canonical CLI form.

One subcommand → one slice — this slice is structural rather than verb-shaped, so the "one subcommand" rule applies loosely: the slice ships **one new top-level `dec product` subcommand tree** (with N child verbs re-exported from product-cli) plus the workspace plumbing that makes it work. Subsequent product-cli evolution lands in normal feature slices that touch `crates/product-cli/` directly.

## Functional Specification

### Inputs

- The current standalone `product-cli` repo at `github.com/Hafeok/product-cli` at the commit that will become the absorption point (the latest released version unless a specific commit is named in the slice's PR description).
- The current decision-cli workspace (this repo) at `main`.
- An operator with write access to both repos for the duration of the absorption.

### Outputs

- `crates/product-cli/` directory in this repo containing the full source of product-cli, with git history preserved via `git subtree`.
- Updated workspace `Cargo.toml` declaring `crates/product-cli` as a workspace member.
- `crates/decision-cli/Cargo.toml` gains `product-cli = { path = "../product-cli" }`.
- New module `crates/decision-cli/src/features/product_cmd/` exposing the `dec product *` subcommand tree as a re-export of product-cli's clap definitions.
- Updated `crates/decision-cli/src/core/mcp/` registering product-cli's MCP tool surface alongside dec's existing tools.
- A new `crates/product-shim/` crate producing the standalone `product` binary, deprecated, delegating to the same Rust API.
- An ADR note on the github.com/Hafeok/product-cli README marking it as a read-only archive pointing at this workspace.
- One smoke-test CI workflow that runs `cargo build --workspace` and `cargo test --workspace` to confirm the absorption builds clean.

### State

- New on-disk: `crates/product-cli/` (subtree merge); `crates/product-shim/` (new crate, ~10 lines).
- Updated on-disk: workspace `Cargo.toml`, `crates/decision-cli/Cargo.toml`, decision-cli's `src/features/` (one new module), decision-cli's `src/core/mcp/` (extended).
- Preserved on-disk: `.product/` and `.dec/` layouts are unchanged — operators with existing checkouts see no migration prompt.
- No schema migration required for existing artifacts.

### Behaviour

#### Phase 1 — git subtree absorption

```
# from the decision-cli repo root
git remote add product-cli-archive https://github.com/Hafeok/product-cli.git
git fetch product-cli-archive
git subtree add --prefix=crates/product-cli product-cli-archive main --squash=false
```

Notes:
- `--squash=false` preserves the full commit history under `crates/product-cli/`.
- The commit message records the absorption point's product-cli SHA + tag so a future reader can map back.
- After this phase, `crates/product-cli/` exists with product-cli's source intact but is **not yet wired into the workspace**.

#### Phase 2 — Cargo workspace wiring

1. Edit the root `Cargo.toml` to add `"crates/product-cli"` to the workspace `members` array.
2. Reconcile product-cli's Cargo.toml with workspace conventions: shared dependencies (oxigraph, serde, anyhow, etc.) move to `[workspace.dependencies]`, product-cli's `Cargo.toml` references them via `dep = { workspace = true }`. Avoid version drift.
3. `cargo build --workspace` — must succeed at this point. product-cli is now a workspace crate; nothing depends on it yet.
4. Edit `crates/decision-cli/Cargo.toml`: add `product-cli = { path = "../product-cli" }`.
5. `cargo build --workspace` again — still succeeds; decision-cli can now `use product_cli::...` even though no code does yet.

#### Phase 3 — `dec product *` subcommand surface

1. Create `crates/decision-cli/src/features/product_cmd/mod.rs`. The module exposes a single function `register(cmd: clap::Command) -> clap::Command` that adds a `product` subcommand tree by composing product-cli's existing clap definitions. **Re-export, not copy** — product-cli's clap tree is the source of truth.
2. Wire the registration in `crates/decision-cli/src/main.rs` (or its existing CLI scaffold).
3. Each verb under `dec product *` delegates to the corresponding product-cli handler function via direct Rust call. No subprocess, no IO besides what product-cli itself does (stdout, file writes).
4. Output formatting is identical to standalone `product` — the test suite asserts byte-for-byte stdout equality for a representative set of verbs (see TC-176).

#### Phase 4 — MCP server merge

1. decision-cli's existing MCP server ([FT-034](FT-034)) lives in `crates/decision-cli/src/core/mcp/`. Locate the tool-registration function.
2. Import product-cli's MCP tool definitions (the `product_*_*` tool set).
3. Register both sets on the same MCP server. Tool name uniqueness is asserted at registration time (panic on collision); current sets have no overlap.
4. The standalone product-cli MCP binary continues to exist (compiled from `crates/product-cli/` directly) but operators are recommended to use the combined `dec mcp` server going forward.

#### Phase 5 — deprecation shim for the standalone `product` binary

1. Create `crates/product-shim/` — a small new crate that builds a `product` binary. The binary is a clap-args passthrough: it parses its args, prints `"warning: 'product' is deprecated; prefer 'dec product <verb>'"` to stderr, and invokes the same Rust handler `dec product` would invoke.
2. The shim's `Cargo.toml` declares `product-cli = { path = "../product-cli" }` and `clap`.
3. The shim is published as the new home for the `product` binary name; `cargo install --path crates/product-shim` produces the same binary operators have today, with a deprecation warning.
4. Deprecation window: **the shim is removed in a slice no earlier than 90 days after FT-105 lands**, giving operators time to update muscle memory and scripts. Removal is its own one-line feature, not tracked here.

#### Phase 6 — named-graphs storage capability

This phase is **optional and additive** within this slice — if a sibling feature wants to start querying across the orchestration store and the product graph, the substrate is in place.

1. The orchestration store (`.dec/store/`) and the product graph (`.product/graph/`) remain on-disk as separate Turtle/N-Quads files (no change).
2. Add a `crates/decision-cli/src/core/graph/multi_named.rs` helper that constructs an in-memory oxigraph store loaded with both, projecting `.product/` into the named graph `<https://decision-cli.dev/ns#product>` and `.dec/store/` into `<https://decision-cli.dev/ns#orchestration>`.
3. Existing callers (today's single-store readers) are **unchanged**. The helper is opt-in for new callers that want cross-layer queries.
4. No migration; no on-disk changes.

#### Phase 7 — archive the standalone repo

1. Update github.com/Hafeok/product-cli's README to point at this workspace and declare the repo read-only.
2. Lock the repo's main branch (no further pushes accepted).
3. Tag the absorption-point commit (e.g. `v0.X.Y-archive`) so the pre-absorption history has a recoverable handle.
4. (Not in this slice's scope to actually execute, since it requires repo-admin access on the external repo; this slice surfaces the action as a manual step in the PR description.)

### Invariants

- **SDP boundary preserved.** `crates/product-cli/Cargo.toml` lists **no** dependency on `decision-cli` or `oxi-events`. Asserted by a CI check (a fitness test that grep'ps `crates/product-cli/Cargo.toml` for these names — see TC-179).
- **No on-disk migration required.** Existing checkouts of decision-cli (with their `.dec/` directories) and existing product-cli adopters (with their `.product/` directories) work unchanged after the workspace adopter installs the new binary. The named-graphs projection is in-memory, not on-disk.
- **Output parity.** For every product-cli verb that exists today, `dec product <verb> <args>` produces byte-identical stdout to `product <verb> <args>` for the same inputs. Asserted for a representative subset of verbs (TC-176).
- **MCP tool uniqueness.** Combined MCP server registers product-cli and decision-cli tools without name collisions; collision is a compile-time panic.
- **Subprocess shim continues to work.** The deprecated `product` binary produces the same exit codes and stdout as the standalone product-cli binary did — operators' existing scripts continue to function during the deprecation window.
- **Git history preserved.** `git log crates/product-cli/` returns the full pre-absorption history. Asserted via `git log --oneline | wc -l` snapshot in CI.
- **One CI pipeline.** A single `cargo test --workspace` run exercises both halves; product-cli's existing test suite is included.
- **Reversibility.** At any point during the deprecation window, an operator can extract product-cli back to a standalone repo via `git subtree split --prefix=crates/product-cli` and publish the result. The absorption is not a one-way door.

### Error handling

- **Cargo dependency conflict** (decision-cli and product-cli pin different versions of a shared dep) → resolved by promoting the higher version to `[workspace.dependencies]` and using `workspace = true` in both crates. If the versions are semver-incompatible, the slice halts and the version conflict is resolved as its own task before continuing.
- **MCP tool name collision** → compile-time panic at registration. Resolution is to rename one of the colliding tools in its source crate before the slice can merge.
- **Subtree merge conflict** (file in `crates/product-cli/` already exists in this repo at the same path) → would be surprising since this repo has no `crates/product-cli/` today; if it appears, resolve manually and document in the PR.
- **Shim invocation against a verb that doesn't exist in product-cli** → shim exits 1 with `unknown subcommand`, same as standalone product-cli would.

### Boundaries

- **In scope.** The seven phases above; the workspace plumbing; the `dec product` subcommand tree; the MCP merge; the deprecation shim; the named-graphs helper (substrate only, no callers); a CI workflow that asserts the absorbed shape; the README update on the standalone repo (as a documented manual step). Output-parity tests for a representative subset of verbs.
- **Out of scope.** Merging the on-disk layouts of `.product/` and `.dec/` (deliberately preserved separate per ADR-067). Refactoring either codebase to share internal modules (the crate boundary is the chokepoint; internal sharing is a later concern if patterns emerge). Removing the deprecated `product` binary (a later one-line slice once the deprecation window passes). Removing ADR-009 from the catalog (it's superseded in part, not deleted — operators reading ADR-009 should be directed forward to ADR-067, but the historical record stays). Migrating existing cross-repo specs (FT-076, FT-104) into intra-repo form — those specs already describe what to build; once product-cli is in the workspace, their implementation lands as normal slices touching `crates/product-cli/`. Promoting any product-cli verb to a first-class `dec *` verb without the `product` prefix (e.g. `dec feature show` instead of `dec product feature show`) — that's a UX call for a later slice once the absorption is settled.

## Out of scope

- On-disk layout merge.
- Internal-module refactoring across the crate boundary.
- Standalone `product` binary removal.
- Deletion of ADR-009.
- Cross-repo spec re-authoring.
- First-class `dec *` aliasing.
- Documentation overhaul (a later doc-pass updates CLAUDE.md and `decision-cli-slice-1-bounds.md` to reflect the absorbed shape; not blocking).
- Re-running the FT-097..FT-104 verification graphs against the new shape (a separate verification sweep, not part of the structural absorption).
