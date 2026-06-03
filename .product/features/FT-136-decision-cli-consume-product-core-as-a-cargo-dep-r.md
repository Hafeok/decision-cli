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
- TC-332
- TC-333
- TC-334
- TC-335
- TC-336
- TC-337
- TC-338
- TC-339
- TC-340
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
3. **product_core API reference** (verified against upstream HEAD `5fad7aa11ca8787ff74e87bb00e1cc0bdfb8b2c1`). `product_core` does NOT expose `feature_show`-style entry points — it exposes typed artifacts (`Feature`, `Adr`, `TestCriterion`) and a `KnowledgeGraph` with `pub` field accessors. The adapter does its own rendering against the typed structs.

   **Canonical load sequence** (do this once per `dec product *` invocation):

   ```rust
   use product_core::{
       config::ProductConfig,
       context,
       error::Result,
       gap,
       graph::{full_check, KnowledgeGraph},
       parser,
       root,
       types::{Adr, Feature, TestCriterion},
   };

   // Resolve the active .product/ root (--root flag, then PRODUCT_ROOT env, then walk-up).
   let repo_root = root::resolve_active()?;
   let config    = ProductConfig::load(&repo_root)?;

   // Resolve per-artifact-type subdirs (respects config.paths.* overrides).
   let features_dir = config.resolve_path(&repo_root, &config.paths.features);
   let adrs_dir     = config.resolve_path(&repo_root, &config.paths.adrs);
   let tests_dir    = config.resolve_path(&repo_root, &config.paths.tests);
   let deps_dir     = config.resolve_path(&repo_root, &config.paths.dependencies);
   let patterns_dir = config.resolve_path(&repo_root, &config.paths.patterns);

   // Load all artifact types.
   let loaded = parser::load_all_full(
       &features_dir, &adrs_dir, &tests_dir,
       Some(&deps_dir), Some(&patterns_dir),
   )?;

   // Build the canonical graph.
   let graph = KnowledgeGraph::build_full(
       loaded.features, loaded.adrs, loaded.tests,
       loaded.dependencies, loaded.patterns,
   );
   ```

   **`KnowledgeGraph` read surface** (all fields are `pub`):

   ```rust
   pub struct KnowledgeGraph {
       pub features:     HashMap<String, Feature>,
       pub adrs:         HashMap<String, Adr>,
       pub tests:        HashMap<String, TestCriterion>,
       pub dependencies: HashMap<String, Dependency>,
       pub patterns:     HashMap<String, Pattern>,
       // ... edges / forward / reverse used internally by graph algorithms
   }

   pub struct Feature { pub front: FeatureFrontMatter, pub body: String, pub path: PathBuf }
   pub struct Adr     { pub front: AdrFrontMatter,     pub body: String, pub path: PathBuf }
   // FeatureFrontMatter / AdrFrontMatter expose id, title, phase, status, depends_on, adrs, tests, etc.

   impl KnowledgeGraph {
       pub fn build_full(features, adrs, tests, deps, patterns) -> Self;
       pub fn stats(&self) -> graph::GraphStats;
       pub fn all_ids(&self) -> HashSet<String>;
       // ... plus traversal helpers used by full_check / gap / context
   }
   ```

   **Per-verb wiring** — each verb loads the graph as above, then:

   | `dec product` verb       | Read pattern                                                                          |
   |--------------------------|---------------------------------------------------------------------------------------|
   | `feature show <ID>`      | `graph.features.get(id)` → render `Feature.front` + `Feature.body`                    |
   | `feature list`           | iterate `graph.features.values()`, filter on phase/status, render one row each        |
   | `feature next`           | use `product_core::feature::depends_on` helpers + `graph::types::FeatureNextResult`   |
   | `adr show <ID>`          | `graph.adrs.get(id)` → render `Adr.front` + `Adr.body`                                |
   | `adr list`               | iterate `graph.adrs.values()`, render one row each                                    |
   | `context <ID>`           | `product_core::context::bundle_feature(&graph, id, ...)` (FT) or `context::bundle_adr` (ADR) |
   | `preflight <ID>`         | `product_core::gap::check::check_feature_dep_gaps(&graph, id)` → `Vec<GapFinding>`    |
   | `graph check`            | `product_core::graph::full_check::run(&graph, &config, &repo_root)` → `CheckResult`   |
   | `graph stats`            | `graph.stats()` → `product_core::graph::GraphStats`                                    |

   Source-of-truth files in the upstream repo (browse these to confirm the API hasn't drifted before re-pinning):

   - `product-core/src/parser.rs:178+` — `load_all`, `load_all_with_deps`, `load_all_full`.
   - `product-core/src/graph/model.rs:55+` — `KnowledgeGraph` struct + `build_full` impl.
   - `product-core/src/graph/types.rs` — `GraphStats`, `FeatureNextResult`, `PhaseGateStatus`, `ImpactResult`.
   - `product-core/src/graph/full_check.rs` — `pub fn run(&graph, &config, root) -> CheckResult`.
   - `product-core/src/gap/mod.rs` — re-exports `check::{check_all, check_feature_dep_gaps, gap_stats}` and the `GapFinding`/`GapReport` types.
   - `product-core/src/context/mod.rs` — re-exports `bundle_feature`, `bundle_feature_with_product`, `bundle_adr`.
   - `product-core/src/types.rs:365+` — `Feature`, `Adr`, `TestCriterion`.
   - `product-core/src/root.rs` — `resolve_active`, `RootSource`.

4. **Renderers.** `product_core` exposes typed artifacts but does NOT expose `product feature show`-style text renderers (those live in `product-cli/src/commands/feature.rs:render_feature_show_text` upstream — a binary-side concern). decision-cli implements equivalent renderers in `crates/decision-cli/src/features/product_cmd/render.rs` (or per-verb sub-modules). Byte-for-byte parity with upstream `product *` stdout is NOT an invariant — TC-329 through TC-339 assert *behavioural* parity (substring match on id / title / phase / status). Adopt the upstream renderer shape where practical so operators familiar with the standalone CLI see a familiar layout.
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
