---
id: FT-167
title: 'decision-cli: Extract dec-ontology crate — pure domain types, vocab, and SHACL shapes at the center of the workspace'
phase: 5
status: planned
depends-on: []
adrs:
- ADR-086
- ADR-072
tests:
- TC-411
- TC-412
- TC-413
domains: []
domains-acknowledged:
  ADR-083: ADR-083 governs tech-detail binding levels for archetypes. FT-167 is a pure relocation of existing ontology/vocab modules; no tech detail is bound at any level.
  ADR-081: ADR-081 governs CLI enumerate/lookup verb pairs. FT-167 is a pure source-tree relocation (core/ontology + core/vocab into crates/dec-ontology) adding no CLI verbs; the dec command surface is byte-identical before and after.
  ADR-084: ADR-084 mandates seam audits for archetypes. FT-167 ships no archetype; the E102 seam-audit gate lands with FT-147 in the crate this slice creates.
  ADR-082: ADR-082 governs archetype-layer semantics. FT-167 only prepares the crate the archetype artifact types will land in (FT-147–FT-152); it implements no archetype behaviour itself.
---

## Description

First migration slice for [ADR-086](ADR-086). Extracts the pure domain layer out of `crates/decision-cli/src/core/` into a new workspace crate `crates/dec-ontology/` — typed artifact definitions, IRI vocabulary modules, embedded SHACL shape files, and the parser/emitter pairs that convert between quad iterators and structs.

This is the center of the stable dependency graph. The crate's load-bearing property is what it *cannot* do: its dependency tree contains no store (`oxigraph`), no async runtime (`tokio`), no HTTP (`axum`/`reqwest`), no CLI (`clap`), and no workspace crate. It speaks `oxrdf` model types (`NamedNode`, `Quad`, `Literal`) natively, because per [ADR-002](ADR-002) IRIs and quads are the domain vocabulary — but it can only describe graph content, never touch a store.

Landing this before [FT-147](FT-147)–[FT-152](FT-152) means the archetype layer's artifact types (Archetype, ApplicationContract, InfrastructureContract, TaskType, Cell, SeamAudit, ArchetypeAudit) are written once, in their final home, and the `add-artifact-type` cluster ([FT-141](FT-141)) emits into the new crate from its first non-prototype run.

## Functional Specification

### Inputs

- `crates/decision-cli/src/core/ontology/` — the typed artifact modules (session, goal, dispatch, feedback, capability, verification bench/graph, worker image, etc.) with their parsers, emitters, and embedded SHACL `.ttl` shape files.
- `crates/decision-cli/src/core/vocab/` — the IRI vocabulary modules.
- The workspace `Cargo.toml` — gains the new member and a pinned `oxrdf` workspace dependency matching the version re-exported by `oxigraph 0.4`.
- The feature_spec bodies of [FT-147](FT-147), [FT-148](FT-148), [FT-149](FT-149), [FT-150](FT-150), [FT-151](FT-151), [FT-152](FT-152) and the `task-types.toml` cell `output_path` values ([FT-166](FT-166)) — all currently pointing at `crates/decision-cli/src/core/ontology/…`.

### Outputs

- `crates/dec-ontology/` — new workspace member with `Cargo.toml` declaring only: `oxrdf`, `serde`, `serde_json`, `thiserror`, `chrono`, `uuid` (subset as actually needed). The relocated modules keep their internal structure (`ontology/…`, `vocab/…`, `shapes/…`).
- Facade re-exports in `crates/decision-cli/src/core/mod.rs`: `pub use dec_ontology::ontology;` and `pub use dec_ontology::vocab;` (exact shape may vary) so existing `crate::core::ontology::…` / `crate::core::vocab::…` import paths in features, core, and tests keep compiling without per-call-site churn.
- Amended output paths in the FT-147–FT-152 spec bodies and `task-types.toml`: `crates/decision-cli/src/core/ontology/…` → `crates/dec-ontology/src/ontology/…` (and vocab accordingly).
- `scripts/checks/dec-ontology-purity.sh` flips from exit 2 (crate absent) to exit 0.

### State

- No graph-resident state changes. No SHACL shape content changes. No IRI changes. This is a source-tree relocation with identical runtime behaviour.
- Workspace `Cargo.toml` member list grows by one; `oxrdf` pinned in `[workspace.dependencies]`.

### Behaviour

1. Create the crate, move `core/ontology/` and `core/vocab/` into it, replacing `oxigraph::model::…` imports with `oxrdf::…` (identical types — oxigraph re-exports oxrdf).
2. Any module currently in `core/ontology/` that genuinely needs the store, tokio, or IO does **not** move — it stays behind in `decision-cli::core` (or moves later to dec-graph/dec-harness per [FT-168](FT-168)/[FT-169](FT-169)), and the split point is recorded in the commit message. The purity constraint decides placement, not the current directory listing.
3. Add the facade re-exports; the full workspace must compile with no changes to feature-slice import statements.
4. Amend FT-147–FT-152 output paths and `task-types.toml` cell `output_path` values.
5. `cargo test --workspace` green; `product verify --platform` green (the topology and purity TCs flip from warning to pass).

### Invariants

- `dec-ontology`'s resolved dependency tree contains none of: `oxigraph`, `tokio`, `axum`, `reqwest`, `clap`, `anyhow`, `oxi-events`, `decision-cli`, `product-core` (enforced by `scripts/checks/dec-ontology-purity.sh`).
- Round-trip behaviour of every parser/emitter pair is byte-identical before and after the move (existing tests relocate with their modules and must pass unmodified).
- No feature slice import statement changes in this slice (facades absorb the move).

### Error handling

- No new runtime error surface. Build-time: a forbidden dependency in `dec-ontology` is a compile/check failure surfaced by the purity TC (exit 1) and by Cargo itself.

### Boundaries

- `dec-ontology` never imports from any workspace crate. All workspace crates may import from it.
- SHACL *enforcement* (validation at write time, [ADR-041](ADR-041)) is out of this crate — shapes live here as data; the chokepoint that applies them stays with the store wrapper ([FT-168](FT-168)).

## Out of scope

- Extracting graph/store machinery ([FT-168](FT-168)) or dispatch/drive machinery ([FT-169](FT-169)).
- Any change to artifact-type semantics, SHACL constraints, or IRI vocabulary content.
- Burning down the facade re-exports (opportunistic, later, per ADR-086).
- The archetype-layer artifact types themselves ([FT-147](FT-147)–[FT-152](FT-152)) — this slice prepares their home, nothing more.