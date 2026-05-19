---
id: ADR-016
title: Vertical-slice architecture with compile-time SDP enforcement
status: proposed
features: []
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
source-files:
- scripts/checks/vertical-slice-imports.sh
- scripts/checks/vertical-slice-layout.sh
---

## Context

decision-cli's Rust workspace today is organised as two crates — `oxi-events` and `decision-cli` — with the second crate's source flattened under `crates/decision-cli/src/` as a row of sibling modules (`init`, `implement`, `events`, `health`, `session_inspect`, `scope`, `stream_writer`, `ontology`, `bundled`, `finalize`, `vocab`). The top-level boundary (`oxi-events` vs `decision-cli`) is governed by [ADR-001](ADR-001) and the Stable Dependency Principle: `oxi-events` has no awareness of DDD vocabulary, and `decision-cli` builds on top of it.

That boundary is the *only* enforced layering in the workspace. Inside `decision-cli` there is no compile-time distinction between **stable core machinery** (the ontology, the scope object, the stream writer, the vocab, the orchestration store wiring) and **feature-volatile machinery** (the slice-1 commands themselves: `dec init`, `dec implement`, `dec health`, `dec events`, `dec session`). Today's siblings can `use` each other freely.

As the slice surface grows (FT-016, FT-017, future slices), this flatness will produce two failure modes:

1. **Cross-feature coupling creeps in by accident.** `implement` already pulls from `init`'s helpers and from `scope`. Tomorrow's `dec dispatch` and `dec watch` features will be tempted to reach into `implement`'s internals because everything is `pub mod`. There is no compile-time fence to reject the reach. Conway-style, the codebase will drift toward a single "decision-cli" blob.

2. **LLM-driven implementation degrades.** [ADR-013](ADR-013) makes the same observation about file size: when context is assembled for a feature, every module the feature can touch becomes potential noise. A flat module list with no stability gradient means a feature for `dec watch` pulls context for `implement`, `events`, `session_inspect`, etc., because the compiler permits it. Bounded scope is a *context quality* constraint, not just a code-aesthetic one.

The standard remedy in component-architecture vocabulary is the vertical slice with the Stable Dependency Principle threaded through the module tree: each layer depends only on layers below; volatile features can be deleted, replaced, or added without touching the stable core; cross-feature imports are a compile error, not a code-review comment.

This ADR codifies that pattern for decision-cli's Rust workspace, inheriting and extending the boundary [ADR-001](ADR-001) draws between `oxi-events` and `decision-cli`.

## Decision

Adopt a four-layer vertical-slice architecture with **compile-time SDP enforcement** via Rust module visibility. From bottom (most stable) to top (most volatile):

```
external substrate          : oxigraph, tokio, axum, serde, tracing
        ↑
oxi-events (crate)          : graph event substrate — ADR-001 boundary
        ↑
decision-cli::core          : ontology, vocab, store wiring, scope,
                              stream_writer, bundled definitions,
                              session/event read models
        ↑
decision-cli::features::*   : one module per vertical slice
                              (init, implement, health, events,
                              session_inspect, finalize, …)
        ↑
decision-cli/src/main.rs    : thinnest possible wiring layer —
                              clap parsing, dispatch into features
```

### The four rules

1. **A layer may depend only on layers strictly below it.**
   - `core` may depend on `oxi-events` and external crates. It MUST NOT depend on any `features::*` module.
   - `features::<slice>` may depend on `core`, `oxi-events`, and external crates. It MUST NOT depend on any other `features::*` module.
   - `main.rs` may depend on `features::*` (to dispatch into them) and on `core` for primitives it must thread (e.g. `workdir` resolution). It MUST NOT contain feature logic.

2. **Features are siblings, not friends.** No `features::implement` → `features::init` import. If two features genuinely share machinery, that machinery is promoted into `core` first; the feature modules then depend on `core`, not on each other. Promotion is a deliberate act that the request log captures.

3. **Visibility is the fence.** Each feature module exposes a deliberately narrow public surface — typically an args struct, a result/outcome struct, and a `run` function — using `pub` only on those items. Internals use `pub(super)` / `pub(crate)` so the compiler rejects cross-feature reach at build time. `core` exposes its surface with `pub`, internals stay `pub(crate)` or module-private.

4. **`main.rs` is wiring only.** `main.rs` contains the `clap` derive trees, the top-level `match`, and the calls into `features::<slice>::run(...)`. No business logic, no SPARQL strings, no formatting helpers beyond minimal arg adaptation. If `main.rs` grows past ~250 lines, that is a smell to be addressed by moving formatting into the relevant feature module — not by lifting the cap.

### Layout

```
crates/decision-cli/src/
  lib.rs                    # crate root — re-exports the public surface
                            # (per-feature args/outcome structs, core types)
  main.rs                   # thinnest wiring layer (ADR-013 §Rule 3 still binds)
  core/
    mod.rs                  # core surface
    ontology.rs             # embedded ontology + SHACL
    vocab.rs                # IRI vocabulary
    bundled.rs              # bundled ValueAction / template lookup
    scope.rs                # ActiveScope + goal validation
    stream_writer.rs        # StreamWriter wrapper (ADR-005)
    store.rs                # orchestration store load / dump helpers
    sparql.rs               # term-extraction utilities (term_iri_string et al.)
  features/
    mod.rs                  # `pub mod init; pub mod implement; …` — flat list
                            # of slice modules; NO cross-imports between siblings.
    init/
      mod.rs                # pub run(...), pub InitArgs, pub InitOutcome, pub InitError
      definition.rs         # pub(super) — internal helpers
      ...
    implement/
      mod.rs                # pub run(...), pub ImplementArgs, pub ImplementOutcome
      bundle.rs             # pub(super)
      worker.rs             # pub(super)
      ...
    health/
      mod.rs
    events/
      mod.rs
    session_inspect/
      mod.rs
    finalize/
      mod.rs
```

Slice 1's existing module names (`init`, `implement`, `health`, `events`, `session_inspect`, `finalize`) become first-class feature directories under `features/`. The stable primitives (`ontology`, `vocab`, `bundled`, `scope`, `stream_writer`) collect under `core/`. New shared helpers (e.g. the `term_iri_string` / `term_literal_string` pair currently inlined in `main.rs`, the `.dec/store/orchestration.nq` load dance repeated across commands) move into `core::sparql` / `core::store` so feature modules can import them rather than re-implement them.

### Compile-time enforcement

The fence is the Rust module system itself:

- `features/mod.rs` exposes `pub mod init;` etc., but nothing inside any sibling module is reachable to its peers because the only ancestor that *could* serve as a re-export point is `features` itself, and `features` declares no `pub use init::…`. Sibling features therefore have no path to each other's types in the module tree.
- `core/mod.rs` re-exports the stable surface (`pub use ontology::*;` etc.) and is the import target for every feature.
- The crate's `lib.rs` re-exports for `main.rs` and integration tests:
  ```rust
  pub mod core;
  pub mod features;          // exists so tests/main can reach feature::run
  pub use features::{init, implement, health, events, session_inspect, finalize};
  ```
- A cross-cutting TC (see "Mechanical check" below) audits `features/*/` modules for forbidden imports (`use crate::features::<other>::*` or `use super::super::<sibling>::*`). This catches any cleverness that tries to route around module privacy via the crate root.

### Promotion rule for shared code

When a feature genuinely needs machinery that lives in another feature, the workflow is:

1. **Stop.** A cross-feature import is a structural smell.
2. **Identify the stable abstraction.** What does the shared code *really* compute? Is it a SPARQL helper, a path resolver, a serialization shape?
3. **Move it to `core/`.** Land the move in a single request with a one-line ADR-016 reference in the commit message: `[ADR-016] Promote <thing> from features::<x> to core::<y>`.
4. **Both features import from `core/`.** The original feature gives up its now-promoted code; the new feature picks it up via the same `core` path.

Promotion is the only allowed route to sharing. The request log captures every promotion, which gives us a falsifiable signal: if `core/` grows by a feature every release, the layering is doing its job; if features start trying to reach sideways and the audit TC fires, the design needs rework.

### Mechanical check

Enforced by a cross-cutting TC under the [ADR-014](ADR-014) convention:

- `scripts/checks/vertical-slice-imports.sh` walks `crates/decision-cli/src/features/*/**/*.rs`, greps for `use crate::features::` and `use super::super::` patterns that resolve to a *different* feature module, and exits 1 on the first hit. Exits 0 on a clean tree.
- The TC carries `validates.adrs: [ADR-016]` and runs as part of `product verify --platform` per ADR-014.

### Interaction with existing ADRs

- **ADR-001 (`oxi-events` as separate crate under SDP).** Unchanged and reinforced. `oxi-events` sits below `core` in the stack; the layering rules apply transitively (`features::*` cannot bypass `core` to reach into `oxi-events` internals).
- **ADR-013 (Code Structure and Quality Standards).** Unchanged. Rule 3 (Module Decomposition) still bounds per-file responsibilities; the new layout *refines* the rule's expected canonical structure rather than replacing it.
- **ADR-005 (Value-stream scope enforced at command time).** Unchanged. `StreamWriter` lives in `core/`; every feature dispatches through it.
- **ADR-011 (CLI shape).** Unchanged. `main.rs` keeps its `clap` derive tree.

## Rejected alternatives

- **Keep the flat module layout, lean on code review.** The current shape relies on reviewers spotting that `features::implement` shouldn't reach into `features::events`. This works in slice 1 (small surface, single author) and stops working the moment we have multiple LLM-authored features landing concurrently. Rejected: code-review enforcement is the failure mode this ADR exists to prevent.

- **Promote every feature into its own crate.** "Each feature is a Rust crate" is the strongest possible SDP fence — cargo enforces it directly. Rejected for slice 1 because per-feature crates impose a real workspace cost (separate `Cargo.toml`, separate target subdir, slower test cycles) that buys us little over module-level `pub(super)` visibility for features at the current size. Revisit if a feature genuinely needs its own dependency graph or if the audit TC starts firing on legitimate use cases that the module fence can't express.

- **Use a single `features/lib.rs` that re-exports all feature symbols.** Tempting because it makes feature surfaces visible from anywhere in the crate. Rejected: that's exactly the routing-around-privacy hole the design must prevent. The whole point is that sibling features have *no* import path to each other.

- **Use clippy lints / cargo-modules / custom procedural macros to enforce.** Rejected for the same reasons ADR-013 rejected a custom clippy lint: shell + grep is auditable in 30 seconds; clippy plugins are brittle and require rustc-internal knowledge. The Rust module system already does most of the work; the audit script is a thin belt-and-braces backup.

- **Defer the migration until slice 2 forces it.** Rejected: every additional slice-1 commit adds more flat-layout sediment to migrate. Doing it now is a one-time cost; doing it later compounds.

- **Split `core/` into multiple sub-namespaces (`core::onto`, `core::store`, `core::scope`).** Rejected for now — `core/` is small enough that further internal partitioning is premature optimization. Revisit when `core/` itself grows past ~8 modules.

## Consequences

**Positive:**

- Cross-feature coupling becomes a compile error, not a review comment. The fence runs in CI without human attention.
- Slice 2 and later can add new feature modules under `features/` with no risk of perturbing the slice-1 surface. Each feature is independently deletable.
- LLM context bundles get sharper: a feature's `product context` no longer pulls in unrelated sibling-feature internals because they aren't reachable.
- `main.rs` shrinks. The current ~745-line `main.rs` is already over ADR-013 Rule 3's `main.rs` cap and contains formatting helpers, SPARQL strings, and store-load code that belongs in feature modules and `core/`. Migration brings it back under the cap as a side-effect.
- The architecture becomes legible. A new contributor (human or LLM) reads `features/init/mod.rs` and sees the entire `dec init` slice in front of them, with no need to chase sibling modules to understand it.

**Negative / accepted costs:**

- One-time migration cost: existing modules move under `core/` and `features/`, integration tests update their import paths, `Cargo.toml` does not change but `lib.rs` re-exports do.
- Marginal friction when first identifying a piece of shared machinery: the developer must choose between "this belongs in `core/`" and "this is genuinely feature-local." The promotion-rule procedure (above) makes that choice explicit.
- A small additional check script in `scripts/checks/` to maintain. Treated like any other ADR-014 fitness function.

**Enforcement:**

- Compile-time via Rust module visibility (the four rules above).
- A cross-cutting TC backed by `scripts/checks/vertical-slice-imports.sh` (per ADR-014).
- The promotion rule's audit trail lives in `.product/requests.jsonl` (every promotion is a request that touches at least one `core/*` file).

## Status

Proposed. Governs `crates/decision-cli/`'s internal layering. The migration of slice-1 modules lands as a single tracked feature ([FT-018](FT-018)).
