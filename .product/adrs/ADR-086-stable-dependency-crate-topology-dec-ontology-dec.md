---
id: ADR-086
title: Stable-dependency crate topology — dec-ontology, dec-graph, dec-harness extracted from decision-cli core
status: accepted
features:
- FT-167
- FT-168
- FT-169
supersedes: []
superseded-by: []
domains:
- data-model
scope: cross-cutting
content-hash: sha256:965c97099fe2bf2d6a217353372f7a8a3cea186de7c2dfdea2c9c6c29850a21e
---

## Context

The workspace today is two crates: `oxi-events` and `decision-cli`. [ADR-001](ADR-001) draws the only compiler-enforced boundary (oxi-events knows nothing of DDD vocabulary); [ADR-016](ADR-016) layers the inside of `decision-cli` into `core/` → `features/*` → `main.rs` with module visibility as the fence.

ADR-016 explicitly deferred two things and named the trigger for revisiting both:

1. *"Split `core/` into multiple sub-namespaces … Rejected for now — `core/` is small enough that further internal partitioning is premature optimization. Revisit when `core/` itself grows past ~8 modules."*
2. Per-feature crates were rejected as overkill, with module visibility judged sufficient *"for features at the current size."*

The trigger has fired. `core/` is now ~30 top-level modules, 280 files, ~52k LOC — graph store wiring, ontology types, IRI vocabulary, SHACL shapes, dispatch loop, cluster dispatch, drive planners, worker contract, bundle assembly, MCP plumbing, verification machinery — all in one compilation unit shared with ~42k LOC of feature slices. Three concrete costs:

- **The stability gradient inside `core/` is invisible.** `core/ontology/` (pure data: structs, parsers, emitters, IRI consts) is far more stable than `core/drive/` (planner logic that changes weekly), but nothing distinguishes them. Module visibility cannot express "this part of core must not do IO" or "this part must not depend on tokio."
- **The archetype layer ([ADR-082](ADR-082)–[ADR-085](ADR-085)) is about to add the largest batch of pure domain vocabulary yet** — Archetype, ApplicationContract, InfrastructureContract, TaskType, Cell, Convention, SeamAudit, ArchetypeAudit ([FT-147](FT-147)–[FT-152](FT-152)) — each a struct + SHACL shape + parser + emitter + vocab module. Landing this into an already-overloaded `core/ontology/` deepens the monolith exactly when the catalog layer needs the domain to be the most stable, most legible thing in the workspace.
- **Build feedback degrades.** Touching any feature slice recompiles the full ~94k-LOC crate. Cluster-dispatched cells pay this on every audit round.

The remedy is the same principle already governing `oxi-events`, applied one level down: promote the *stable layers* of `core/` to crates so Cargo — not review, not module visibility — enforces the dependency direction. This is Clean Architecture's useful core (dependencies point inward toward a pure domain) without its ceremony: no repository traits, no use-case layer, no abstracting the graph away. Per [ADR-002](ADR-002) the graph *is* the state; IRIs and quads *are* the domain vocabulary. The domain crate therefore speaks `oxrdf` model types natively — what it must not have is a store, a runtime, or IO.

## Decision

Extract three crates from `decision-cli`'s `core/`, ordered by stability, with the domain at the center of the dependency graph:

```
                oxrdf (RDF model types only)
                  ↑
            dec-ontology            ← THE CENTER. Most stable.
              ↑       ↑
   oxigraph → dec-graph             ← store wrapper, SHACL chokepoint
              ↑       ↑
oxi-events → dec-harness            ← dispatch, drive, workers, bundles
                      ↑
                decision-cli        ← binary: clap, feature slices, MCP shim
```

### Crate contracts

**`dec-ontology` — the domain.** Typed artifact definitions (Session, Goal, Dispatch, Feedback, Capability, VerificationGraph, and the incoming Archetype/TaskType/Cell family), IRI vocabulary modules, SHACL shape files (embedded `.ttl`), parsers (quad-iterator → struct) and emitters (struct → `Vec<Quad>`).
*Allowed dependencies:* `oxrdf`, `serde`, `thiserror`, `chrono`, `uuid`. **Forbidden:** `oxigraph` (the store), `tokio`, `axum`, `reqwest`, `clap`, `oxi-events`, `anyhow`, any crate in this workspace. No IO, no async, no process state. This is structurally enforced: the crate cannot open a store or make a network call because nothing in its dependency tree can.

**`dec-graph` — graph access.** Orchestration store open/load/dump, named-graph management, SPARQL execution helpers, query templates, bundle CONSTRUCT execution, the SHACL-enforced GraphWriter chokepoint ([ADR-041](ADR-041)), stream writer ([ADR-005](ADR-005)).
*Allowed:* `dec-ontology`, `oxigraph`, `oxi-events`, plus runtime crates as needed. **Forbidden:** `dec-harness`, `decision-cli`, `clap`.

**`dec-harness` — orchestration machinery.** Dispatch loop and dispatch sessions, cluster dispatch, drive planners (ship, def-ready, readiness orchestrator), worker contract and worker resolution, subscriptions, role catalog, bundle assembly, capability resolution and escalation.
*Allowed:* `dec-graph`, `dec-ontology`, `oxi-events`, `product-core`, `tokio`, `reqwest`, etc. **Forbidden:** `decision-cli`, `clap`.

**`decision-cli` — the volatile outer ring.** Unchanged in role: clap trees, `main.rs` wiring, the MCP shim, and the vertical feature slices under `features/*`. [ADR-016](ADR-016)'s four rules continue to govern this crate verbatim — features stay modules (sibling-crate-per-feature remains rejected), features never import features, `main.rs` is wiring only.

### Migration mechanics

- **Facade re-exports limit churn.** During and after migration, `decision-cli::core::ontology`, `::core::vocab`, `::core::graph`, etc. remain valid paths as `pub use dec_ontology::…` / `pub use dec_graph::…` facades. Feature slices keep their imports; only the crates' own internals move.
- **One feature_spec per extraction**, landing in stability order: dec-ontology first ([FT-167](FT-167)), then dec-graph ([FT-168](FT-168)), then dec-harness ([FT-169](FT-169)). Each is a pure relocation — no behaviour change, full test suite green before and after, `product verify --platform` green.
- **The archetype-layer slices land in the new home.** [FT-147](FT-147)–[FT-152](FT-152) emit their new artifact types into `crates/dec-ontology/`. FT-167 includes amending those specs' output paths and the corresponding `task-types.toml` cell `output_path` values ([FT-166](FT-166)) so the `add-artifact-type` cluster writes to the new crate.
- **`oxrdf` is pinned workspace-wide** to the version oxigraph 0.4 re-exports, so `dec-ontology`'s model types and `dec-graph`'s store types are identical types.

### Mechanical checks (per ADR-014)

Two new fitness scripts, each carried by a cross-cutting TC:

- `scripts/checks/crate-dependency-topology.sh` — reads `cargo metadata` and asserts the arrows above: no workspace crate depends on `decision-cli`; `dec-ontology` depends on no workspace crate; `dec-graph` does not depend on `dec-harness`. Exits 2 (warning) while the extracted crates do not yet exist, 0/1 once they do.
- `scripts/checks/dec-ontology-purity.sh` — asserts `dec-ontology`'s resolved dependency tree contains none of the forbidden crates (`oxigraph`, `tokio`, `axum`, `reqwest`, `clap`, `anyhow`, `oxi-events`). Exits 2 while the crate does not yet exist.

The existing ADR-016 checks (`vertical-slice-imports.sh`, `vertical-slice-layout.sh`) are unchanged and continue to bind inside `decision-cli`.

### Interaction with existing ADRs

- **[ADR-001](ADR-001)** — unchanged and generalized: the same crate-level SDP that protects `oxi-events` now protects three more boundaries.
- **[ADR-016](ADR-016)** — refined, not superseded. Its intra-crate rules (vertical slices, sibling isolation, promotion rule, main.rs-is-wiring) all still bind inside `decision-cli`. What changes is the destination of promotion: "promote to `core/`" becomes "promote to the appropriate crate" — domain types to `dec-ontology`, graph access to `dec-graph`, orchestration machinery to `dec-harness`. The promotion workflow and audit trail are unchanged.
- **[ADR-041](ADR-041)** — unchanged; the SHACL chokepoint moves to `dec-graph` but remains the single write path.
- **[ADR-077](ADR-077)** — unchanged; `product-core` stays a git dependency, consumed by `dec-harness` and `decision-cli`.
- **[ADR-082](ADR-082)–[ADR-085](ADR-085)** — the archetype layer's artifact types get a stable, pure home before they land.

## Rejected alternatives

- **Full Clean Architecture (ports/adapters, repository traits over the store).** Rejected: per ADR-002 the graph is the state, and oxrdf terms are the domain vocabulary. Trait-abstracting oxigraph away would add indirection with no second implementation in sight. The valuable part of the pattern — a pure center with dependencies pointing inward — is achieved by crate topology alone.
- **One crate per feature slice.** Re-rejected for the same reasons as ADR-016: 35+ feature crates impose workspace cost that module visibility already covers. The volatile ring does not need crate fences; the stable center does.
- **Keep one crate, split `core/` into sub-namespaces only.** Rejected: module namespaces cannot forbid `tokio` in the ontology layer or express "no IO here." The constraint that makes the domain trustworthy is a *dependency-tree* constraint, which only a crate boundary states.
- **A single `dec-core` crate (ontology + graph + harness together).** Rejected: it reproduces today's problem one level up — the dispatch loop and the Archetype struct would still share one dependency tree, so the domain would still transitively carry tokio/oxigraph/reqwest. The three-way split is the minimum that makes the center pure. (Landing order still de-risks this: if after FT-167 the marginal value of splitting graph from harness looks low, FT-168/FT-169 can be re-scoped by amending this ADR.)
- **Defer until after the archetype layer (FT-147–FT-152) lands.** Rejected: those six slices are the largest injection of pure domain vocabulary in the project's history. Landing them in `core/ontology/` and migrating immediately after means writing every struct, shape, parser, emitter, and cluster cell output path twice.

## Consequences

**Positive:**

- The dependency direction toward the domain is enforced by Cargo. A `use tokio::…` in `dec-ontology` is a compile error, not a review comment.
- The archetype catalog layer lands in a crate whose entire dependency closure is data types — the most legible possible home for the artifact vocabulary that cluster cells, workers, and product-cli all need to agree on.
- Incremental builds: feature-slice edits recompile only `decision-cli`; ontology edits recompile in seconds (no rocksdb, no libclang in `dec-ontology`'s tree).
- Context bundles sharpen further: an `add-artifact-type` cluster cell's world is one small pure crate.

**Negative / accepted costs:**

- Three migration features' worth of relocation work, plus amending FT-147–FT-152 output paths and `task-types.toml`.
- A workspace with five crates instead of two; slightly more `Cargo.toml` surface.
- The facade re-exports are a transitional crutch; they should be burned down opportunistically as slices are touched, not preserved forever.

**Enforcement:** compile-time via Cargo dependency declarations; `crate-dependency-topology.sh` and `dec-ontology-purity.sh` as cross-cutting TCs in `product verify --platform`; ADR-016's existing checks unchanged.

## Status

Proposed. Migration lands as [FT-167](FT-167) (dec-ontology), [FT-168](FT-168) (dec-graph), [FT-169](FT-169) (dec-harness), in that order, before the archetype-layer slices FT-147–FT-152 dispatch.