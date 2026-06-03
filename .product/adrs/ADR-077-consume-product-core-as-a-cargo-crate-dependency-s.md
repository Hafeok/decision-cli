---
id: ADR-077
title: Consume product-core as a Cargo crate dependency; supersede subtree-absorption plan
status: accepted
features:
- FT-136
- FT-106
supersedes:
- ADR-067
superseded-by: []
domains:
- api
scope: domain
content-hash: sha256:2315a4b8b23144b218501e9dc6bfb2f0f0e04acaba9756e83d17d1f7717fa228
---

## Context

[ADR-067](ADR-067) decided to absorb product-cli into the decision-cli Cargo workspace via `git subtree add --prefix=crates/product-cli`. [FT-105](FT-105) scoped the implementation. The stated wins were: one workspace, one CI pipeline, one release artifact; cross-repo specs become intra-repo; git history preserved under `crates/product-cli/`; SDP boundary enforced at the crate level.

Two facts have changed since ADR-067 was written:

1. **product-cli has been refactored into a proper multi-crate workspace.** Upstream now ships four crates: `product-core` (the engine — `feature`, `adr`, `context`, `gap`, `graph`, `parser`, `verify`, … — a pure library with no CLI knowledge), `product-mcp` (stdio + HTTP MCP server, depends on product-core), `product-cli` (the `product` binary, depends on both), and `xtask`. ADR-067 was written when product-cli was a monolithic single crate; its absorption plan reflected that monolith and had no notion of "just take the engine".

2. **The subtree merge in FT-105 never actually happened.** `crates/product-cli/` contains a ~280-line stub whose verbs print `"... (stub)"`; `crates/product-shim/` is a 12-line forwarder onto that stub. FT-105's `status: complete` reflects the scaffolding (workspace members, crate names, server.json placeholders) landing — not the source absorption. So nothing about the "absorbed shape" is load-bearing in this repo yet. Re-scoping is a forward design correction, not a rollback of merged code.

The combination is structurally significant: the upstream split moved the SDP-boundary enforcement from "this workspace will carefully avoid importing decision-cli into product-cli" into "product-core is definitionally a library with no decision-cli knowledge". The chokepoint ADR-009 and ADR-067 protect is now a property of the upstream factoring, not something this workspace has to police via a fitness check.

## Decision

**decision-cli consumes `product-core` (and `product-mcp` where MCP-server composition is wanted) as Cargo git dependencies pinned to a commit SHA. The standalone `product-cli` repository remains an independent live repository; no source is vendored, no subtree is merged, no `crates/product-cli/` exists in this workspace.** The deprecation shim plan from ADR-067 §Phase 5 is dropped — the standalone `product` binary continues to be produced by the standalone repo's release flow, not by this one.

Concretely:

### Dependency direction

Same as ADR-067: `decision-cli → product-core → (nothing related to decision-cli)`. The SDP boundary is structurally enforced because `product-core`'s `Cargo.toml` lists no path/git/registry dependency on `decision-cli` or `oxi-events` — a property of the upstream repo, verifiable from cargo's fetched copy.

### Workspace layout

```
.
├── crates/
│   ├── oxi-events/                      (unchanged)
│   └── decision-cli/                    (unchanged in shape;
│                                         gains product-core, product-mcp deps)
└── ...
```

`crates/product-cli/` (the stub) and `crates/product-shim/` (the unused shim) are deleted. The workspace's `Cargo.toml` `members` array loses both entries. `[workspace.dependencies]` adds:

```toml
product-core = { git = "https://github.com/Hafeok/product-cli", rev = "<pinned-sha>" }
product-mcp  = { git = "https://github.com/Hafeok/product-cli", rev = "<pinned-sha>" }
```

`crates/decision-cli/Cargo.toml` lists `product-core = { workspace = true }` and `product-mcp = { workspace = true }`.

### Integration mode (carried forward from ADR-067)

- **Direct Rust API for reads.** `dec product feature show FT-097` calls `product_core::feature::*` in-process. No subprocess.
- **MCP composition.** `dec mcp` builds its registry by registering both `product_mcp::registry::ToolRegistry` and dec's own tools. Tool-name uniqueness is asserted at registration time; collision is a startup-time defect.
- **`dec product *` clap tree is hand-authored** in `features/product_cmd/` on top of `product_core`. There is no clap re-export from upstream (upstream's clap tree lives in the `product-cli` binary crate, which this workspace does not consume). The two CLI surfaces — `dec product *` here, `product *` upstream — are independent; behavioural parity matters, byte-for-byte stdout parity does not.

### CLI binary

The standalone `product` binary is shipped by the standalone repo's release flow. This workspace ships the `dec` binary only. ADR-067 §Phase 5 / FT-105 §Phase 5's deprecation shim is **not built**; `crates/product-shim/` is deleted.

### Upstream evolution

A `product-core` change motivated by a `dec` feature lands in the standalone repo first (one PR there), then this repo bumps the pinned rev (`cargo update -p product-core --precise <new-sha>`). Cross-repo specs do not fully go away, but they collapse to "agree on an API in product-core, then consume it here" — much smaller than ADR-067 anticipated, because the engine is one crate, not a sprawling source tree.

### `[patch]` for local iteration

During concurrent development on both repos, contributors add a gitignored `.cargo/config.toml` `[patch."https://github.com/Hafeok/product-cli"]` override pointing at a local checkout. This is documented in `CONTRIBUTING.md` — not enforced.

## Rejected alternatives

### Land the original ADR-067 subtree absorption against the split

Vendor the entire post-split `product-cli` repo (four crates) under `crates/product-cli/` and list each sub-crate as a workspace member. Rejected — the split made this strictly worse: more workspace members to maintain, more Cargo.toml ceremony, no upside the simpler Cargo-dep approach does not already provide. ADR-067's "one CI pipeline" argument carries less weight when the upstream library is already CI-tested standalone.

### Vendor only `product-core` via subtree

Subtree-merge just `product-core/` into `crates/product-core/`. Rejected — gets the workspace-member overhead for the engine but still requires cross-repo bumps for `product-mcp`. The Cargo-dep approach handles both crates uniformly with less plumbing.

### Publish `product-core` and `product-mcp` to crates.io and use registry deps

Architecturally identical to the git-dep decision; just adds a `cargo publish` step to every upstream release. Rejected for now — `cargo publish` ceremony is wasted ceremony while the only consumer is decision-cli. Revisit if an external consumer appears.

### Keep `crates/product-cli/` as a thin re-export shim instead of deleting

`crates/product-cli/lib.rs` becomes `pub use product_core::*;` so existing import paths keep working. Rejected — there are essentially no internal callers (the stub printed `(stub)` for everything; nothing real depends on it), so the re-export adds an indirection layer for zero callers.

### Defer; live with the stub and rerun ADR-067 later

Rejected — the stub is misleading (FT-105 reads as complete; the code says otherwise) and FT-106 is already conditioned on the absorbed shape. The longer the misalignment persists, the more downstream work treats a stub as the real thing.

## Consequences

### Positive

- **No source duplication.** product-core lives in one place; this workspace consumes it like any other crate.
- **Smaller workspace.** Two crates deleted (`crates/product-cli/`, `crates/product-shim/`); build time drops.
- **No subtree merge to maintain.** Updating the dep is a one-line `cargo update --precise`; no `git subtree pull` ever.
- **SDP boundary enforced by upstream factoring.** product-core is structurally incapable of depending on decision-cli (it's in a separate repo with no such dep); the cross-crate-direction fitness check from ADR-067 §Invariants becomes redundant.
- **Standalone product-cli adoption stays live.** Anyone who wanted standalone product-cli for a non-DDD use case continues to get a maintained crate; the "archive the standalone repo" step from ADR-067 §Phase 7 is dropped.

### Negative / accepted trade-offs

- **Cross-repo PRs return for engine changes.** A `dec` feature needing a `product-core` API change is two PRs in two repos. Mitigated by the small API surface (the engine, not the whole UX layer) and `[patch]` for local concurrent iteration.
- **`git log crates/product-cli/` no longer shows engine history.** Read history in the standalone repo instead.
- **Reproducibility depends on the pinned SHA staying reachable.** GitHub repos can be force-pushed or deleted; mitigated by Cargo's own caching of fetched git deps and (eventually) a crates.io publish once external consumers warrant it.
- **No `dec`-shipped `product` binary.** Users who installed `product` from decision-cli's releases (none today — neither FT-105 nor FT-106 actually shipped one) install it from the standalone repo's releases as before.

### Relationship to [ADR-067](ADR-067) and [ADR-009](ADR-009)

- **ADR-067:** partially superseded. The decision to consume product-cli's engine from decision-cli stays; the *transport* (subtree absorption → Cargo dep) changes. ADR-067's Phases 1, 2, 3, 5, 7 are dropped; Phase 4 (MCP merge) and Phase 6 (named-graphs storage capability) survive in spirit and are re-implemented against the Cargo-dep shape in the follow-on feature.
- **ADR-009:** unchanged. ADR-009's principle ("single interface for humans and LLMs") and SDP direction (`decision-cli → product-cli`) are reaffirmed; the slice-1 subprocess transport note in ADR-009 remains historically accurate.

### [FT-105](FT-105) status

FT-105's scaffolding (workspace members, deprecation-shim crate name, server.json placeholders) is undone by the supersession. FT-105 itself stays in the catalog as historical record of the original intent; the replacement work is tracked under a new feature_spec linked to this ADR.

### [FT-106](FT-106) status

FT-106 (cross-platform cargo-dist release flow + MCP-registry publishing for the absorbed workspace) becomes partially obsolete. Its `product` binary release path no longer applies (the standalone repo's release flow is unchanged). The `dec` binary release path survives but simplifies — only one binary to ship, not two. A follow-on slice re-scopes FT-106 against the new shape; not blocking on FT-106 changing today.

## Status

Proposed. Once the follow-on feature_spec ships and `dec product *` is verifiable against `product_core`, this ADR advances to accepted via the standard supersede ceremony against ADR-067.
