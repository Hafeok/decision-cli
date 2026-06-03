---
id: FT-136
title: 'decision-cli: Consume product-core as a Cargo dep; retire product-cli stub and product-shim'
phase: 4
status: planned
depends-on: []
adrs:
- ADR-077
- ADR-009
- ADR-067
tests:
- TC-328
- TC-329
- TC-330
- TC-331
domains:
- api
domains-acknowledged:
  observability: FT-136 has 4 linked TCs (TC-328 through TC-331) satisfying ADR-072's minimum-coverage rule. ADR-072 spans api + observability; api is a primary domain on this feature. Observability concerns are scoped to the MCP-merge TC (TC-330) which observes mcp-response on a live boot, and the SDP-boundary TC (TC-331) which observes file state on the fetched dep — both surface live state rather than relying on internal invariants. Explicit acknowledgement per ADR-072 review gate.
---

## Description

The implementation slice for [ADR-077](ADR-077)'s decision to consume `product-core` (and `product-mcp`) as Cargo git dependencies rather than absorbing product-cli's source via `git subtree`. The slice replaces the stub `crates/product-cli/` and the unused `crates/product-shim/` with two `[workspace.dependencies]` entries and a real `dec product *` adapter built directly on top of `product_core`'s public Rust API.

The slice is mechanical and bounded: build clean, `dec product feature show FT-XXX` returns the same artifact contents the upstream `product feature show FT-XXX` would (rendering may differ — behavioural parity, not byte parity), and `dec mcp` exposes the union of `product_*` and `dec_*` tools without name collision. Once it lands, the `dec product *` vocabulary CLAUDE.md describes is real-by-Cargo-dep rather than real-by-stub.

One subcommand → one slice — the slice ships a single user-visible top-level verb (`dec product`) with N child verbs forwarded to `product_core` handlers, plus the workspace plumbing and MCP merge that make it work.

## Functional Specification

### Inputs

- The current `decision-cli` workspace at `main`.
- The standalone `product-cli` workspace at a pinned commit SHA on `main`; the SHA is recorded in the PR description and in the `[workspace.dependencies]` entry.
- An operator with `cargo` access and a local clone of both repos for `[patch]`-based iteration.

### Outputs

- `crates/product-cli/` directory deleted (the ~280-line stub).
- `crates/product-shim/` directory deleted (the 12-line forwarder).
- Workspace `Cargo.toml`: `members` array loses both entries; `[workspace.dependencies]` gains `product-core = { git = "https://github.com/Hafeok/product-cli", rev = "5fad7aa11ca8787ff74e87bb00e1cc0bdfb8b2c1" }` and `product-mcp = { git = "...", rev = "5fad7aa11ca8787ff74e87bb00e1cc0bdfb8b2c1" }`.
- `crates/decision-cli/Cargo.toml`: replaces `product-cli = { workspace = true }` with `product-core = { workspace = true }` and `product-mcp = { workspace = true }`.
- `crates/decision-cli/src/features/product_cmd/` rewritten as a clap tree forwarding each verb to `product_core::*` calls. The old stub adapter at `crates/decision-cli/src/cli/product.rs` is deleted.
- `crates/decision-cli/src/core/mcp/` extended to register `product_mcp::registry::ToolRegistry` alongside dec's existing tools; tool-name uniqueness is asserted at registration.
- `CONTRIBUTING.md` (created if absent) documents the `[patch]` workflow for concurrent development.
- `CLAUDE.md` updated to reflect the Cargo-dep shape (the `crates/product-cli/` and `crates/product-shim/` rows disappear; `dec product *` is described as a thin adapter on top of `product_core`).
- A CI smoke-test runs `cargo build --workspace` and `cargo test --workspace` and asserts both pass.

### State

- Removed on-disk: `crates/product-cli/`, `crates/product-shim/`.
- Updated on-disk: root `Cargo.toml`, `crates/decision-cli/Cargo.toml`, `crates/decision-cli/src/features/product_cmd/` (rewritten), `crates/decision-cli/src/cli/product.rs` (deleted), `crates/decision-cli/src/core/mcp/` (extended), `Cargo.lock` (regenerated), `CLAUDE.md`, `CONTRIBUTING.md`.
- Preserved on-disk: `.product/`, `.dec/`, `crates/oxi-events/`, all worker directories, all docs under `docs/`.
- No graph migration; no session-store migration; no on-disk schema change.

### Behaviour

#### Phase 1 — Add the Cargo deps and delete the stub crates

The Cargo wiring and the stub deletion land in **one commit** to avoid an intermediate broken state (the stub `product-cli` crate is referenced by `crates/decision-cli/src/cli/product.rs`; deleting one without the other fails the build).

1. Edit root `Cargo.toml`:
   - Remove `"crates/product-cli"` and `"crates/product-shim"` from `[workspace] members`.
   - Add `product-core = { git = "https://github.com/Hafeok/product-cli", rev = "5fad7aa11ca8787ff74e87bb00e1cc0bdfb8b2c1" }` and `product-mcp = { git = "...", rev = "5fad7aa11ca8787ff74e87bb00e1cc0bdfb8b2c1" }` to `[workspace.dependencies]`.
   - Remove the old `product-cli = { path = "crates/product-cli" }` entry.
2. Edit `crates/decision-cli/Cargo.toml`:
   - Replace `product-cli = { workspace = true }` with `product-core = { workspace = true }` and `product-mcp = { workspace = true }`.
3. `git rm -r crates/product-cli crates/product-shim`.
4. `cargo build --workspace` — fails until Phase 2 rewires the adapter; commit boundary is at Phase 2.

#### Phase 2 — Rewrite `features/product_cmd/` on top of `product_core`

1. Delete `crates/decision-cli/src/cli/product.rs` (the old stub adapter).
2. Recreate `crates/decision-cli/src/features/product_cmd/mod.rs` as a clap tree forwarding to `product_core`. Sketch:
   ```rust
   pub fn build_command() -> clap::Command { /* feature / adr / context / preflight / graph subcommands */ }
   pub fn dispatch(matches: &clap::ArgMatches) -> ExitCode { /* match verb → product_core::* */ }
   ```
3. Each verb delegates to `product_core::*` in-process:
   - `product feature show ID` → `product_core::feature::show(repo_root, ID)` → render to stdout
   - `product feature list` → `product_core::feature::list(repo_root, filters)` → render
   - `product feature next` → `product_core::feature::next(repo_root)` → render
   - `product adr show ID` → `product_core::adr::show(repo_root, ID)` → render
   - `product adr list` → `product_core::adr::list(repo_root)` → render
   - `product context ID` → `product_core::context::assemble(repo_root, ID, depth, target)` → render
   - `product preflight ID` → `product_core::gap::preflight(repo_root, ID)` → render
   - `product graph check` → `product_core::graph::check(repo_root)` → render
   - `product graph stats` → `product_core::graph::stats(repo_root)` → render
4. Renderer choice: where `product_core` exposes a renderer, use it; where the upstream `product` binary builds its own output in `product-cli/src/commands/`, decision-cli implements an equivalent renderer. The slice does not chase byte-for-byte parity with upstream `product *` stdout — that surface is the upstream binary's responsibility.
5. Wire `register(cmd)` into `crates/decision-cli/src/main.rs` (or the existing CLI scaffold) so `dec product` is reachable.
6. `cargo build --workspace` succeeds at the end of this phase; this is the natural commit boundary.

#### Phase 3 — MCP merge

1. Locate the MCP registry construction in `crates/decision-cli/src/core/mcp/`.
2. Import `product_mcp::registry::ToolRegistry` (and any required handlers/types from `product_mcp`).
3. At server-bootstrap time, register product-mcp's tool set into the same registry dec uses for `dec_*` tools. Tool-name uniqueness is asserted at registration; collision is a startup-time panic. Current sets (`product_*` vs `dec_*`) have no overlap.
4. `dec mcp` stdio + HTTP both expose the union. The standalone `product mcp` continues to exist in the standalone repo for operators who want only the product tools — that surface is unchanged.

#### Phase 4 — Documentation

1. Add `CONTRIBUTING.md` (or extend if it exists) with a `[patch]` workflow note:

   > **Working on `product-core` changes alongside `decision-cli`.** Drop a gitignored `.cargo/config.toml` at the workspace root:
   > ```toml
   > [patch."https://github.com/Hafeok/product-cli"]
   > product-core = { path = "../product-cli/product-core" }
   > product-mcp  = { path = "../product-cli/product-mcp" }
   > ```
   > Iterate freely. When ready, commit the `product-core` change upstream, then bump `rev` here via `cargo update -p product-core --precise <new-sha>`.

2. Ensure `.cargo/config.toml` is in `.gitignore` so per-developer overrides do not leak.

3. Update `CLAUDE.md`:
   - "What this project is" — link to product-cli stays; the absorption note is removed.
   - "Where things live" — the `crates/product-cli/` and `crates/product-shim/` rows disappear.
   - "CLI vocabulary" — `dec product <subcommand>` is described as a thin adapter on top of `product_core`'s public API.

#### Phase 5 — CI smoke test

A single CI workflow (or an addition to an existing one) runs:

```bash
cargo build --workspace
cargo test --workspace
```

The workflow asserts both succeed. It also includes a grep step that fails if either `crates/product-cli/` or `crates/product-shim/` reappears (regression guard).

### Invariants

- **SDP boundary preserved.** `product-core`'s `Cargo.toml` has no dependency on `decision-cli` or `oxi-events`. Verifiable from cargo's fetched copy under `~/.cargo/git/checkouts/`. The check is mostly redundant — cargo's resolver would refuse a cycle — but documents the property explicitly.
- **No `crates/product-cli/` or `crates/product-shim/` after this slice.** Asserted by CI (Phase 5 grep step).
- **`dec product *` produces deterministic output given the same `.product/` state.** Behavioural parity (same artifact contents) across runs; byte parity with upstream `product *` is not required.
- **`dec mcp` registers `product_mcp` tools and `dec_*` tools without collision.** Asserted by a test that boots the MCP server, calls `tools/list`, and checks the union plus absence of duplicate names.
- **No worker code changes.** Workers continue talking to `dec mcp` (and optionally `product mcp` directly); their contracts are unaffected.
- **Pinned SHA resolves on landing.** A fitness check fetches the SHA from upstream and asserts it resolves to a commit reachable from `main`.

### Error handling

- **Cargo dep version conflict** with a transitive dependency of `product-core` → resolve by promoting the higher version into `[workspace.dependencies]` or by patching a compatible version. If unresolvable, the slice halts on a separate version-resolution task before continuing.
- **`product-core` API gap** (decision-cli needs a call `product-core` does not expose, e.g. a renderer fn) → the slice halts; the missing API is added upstream first (one PR in the standalone repo), then this slice resumes with the bumped rev.
- **MCP tool name collision** between `product_*` and `dec_*` → startup-time panic; resolution is to rename the conflicting `dec_*` tool on this side (the `product_*` namespace is the established one and renames there would break standalone users).
- **`[patch]` config accidentally committed** → `.gitignore` covers `.cargo/config.toml`; PR review catches anything that slips through.
- **Upstream SHA force-pushed or deleted** → Cargo's local cache continues to work; CI's fitness check fails until a new SHA is pinned. Recovery is a `cargo update --precise <new-sha>` to a reachable commit.

### Boundaries

- **In scope.** The five phases above; the Cargo wiring; the `features/product_cmd/` rewrite on `product_core`; the MCP merge with `product_mcp`; the stub-crate deletions; `CLAUDE.md` updates; `CONTRIBUTING.md` `[patch]` note; the CI smoke test and regression grep.
- **Out of scope.** Re-scoping [FT-106](FT-106) (a separate slice, since FT-106 has not actually shipped a `product` binary from this repo yet). Publishing `product-core`/`product-mcp` to crates.io (deferred). Merging `.product/` and `.dec/` on-disk layouts (preserved separate per ADR-067 carryover; ADR-077 inherits). Removing the standalone `product` binary or archiving the standalone repo (both stay alive). Re-running prior verification graphs against the new shape (separate sweep). Migrating cross-repo specs FT-076 and FT-104 — both have already shipped against the standalone shape; no rework needed. Promoting any `product` verb to a first-class `dec *` verb without the `product` prefix (UX call for later).

## Out of scope

- Re-scoping FT-106 (separate slice).
- Publishing product-core/product-mcp to crates.io.
- On-disk graph/store layout changes.
- Removing the standalone product-cli repo.
- Re-running prior verification sweeps.
- Documentation overhaul beyond the listed sections.
- First-class `dec *` aliasing of product verbs.
- Per-feature performance benchmarking (the dep change is not motivated by perf).
