---
id: FT-168
title: 'decision-cli: Extract dec-graph crate — store wrapper, SPARQL execution, and the SHACL GraphWriter chokepoint'
phase: 5
status: planned
depends-on:
- FT-167
adrs:
- ADR-086
- ADR-072
tests:
- TC-411
- TC-414
domains: []
domains-acknowledged:
  ADR-084: ADR-084 mandates seam audits for archetypes. FT-168 ships no archetype; E102 enforcement arrives with FT-147 through the chokepoint this slice relocates.
  ADR-082: ADR-082 governs archetype-layer semantics. FT-168 relocates graph-access machinery; it implements no archetype behaviour. The GraphWriter chokepoint it moves will later host archetype SHACL shapes (FT-147), unchanged in mechanism.
  ADR-083: ADR-083 governs tech-detail binding levels for archetypes. FT-168 is a pure relocation of store/SPARQL machinery; no tech detail is bound at any level.
  ADR-081: ADR-081 governs CLI enumerate/lookup verb pairs. FT-168 is a pure relocation of graph/store/SPARQL machinery into crates/dec-graph adding no CLI verbs; the dec command surface is unchanged.
---

## Description

Second migration slice for [ADR-086](ADR-086). Extracts the graph-access layer out of `crates/decision-cli/src/core/` into a new workspace crate `crates/dec-graph/`: orchestration store open/load/dump, named-graph management, SPARQL execution helpers and query templates, bundle CONSTRUCT execution, the stream writer ([ADR-005](ADR-005)), and the SHACL-enforced GraphWriter chokepoint ([ADR-041](ADR-041)).

After this slice, the workspace expresses the rule that was previously only convention: everything that touches the store goes through one crate, and that crate depends on the domain ([FT-167](FT-167)'s `dec-ontology`) — never the other way around.

## Functional Specification

### Inputs

- `crates/decision-cli/src/core/` modules: `graph/` (including the GraphWriter SHACL chokepoint), `store.rs`, `sparql.rs`, `queries/`, `bundle/`, `stream_writer.rs`, `stream_writer_validations.rs`.
- `crates/dec-ontology/` from [FT-167](FT-167) — the typed artifacts and shapes this crate validates and persists.
- `crates/oxi-events/` — the mutation/subscription substrate the chokepoint routes through ([ADR-001](ADR-001) boundary unchanged).

### Outputs

- `crates/dec-graph/` — new workspace member. `Cargo.toml` declares `dec-ontology`, `oxigraph`, `oxi-events`, plus runtime crates as needed (`tokio`, `serde`, `thiserror`, `tracing`). **Never** `clap`, never `dec-harness`, never `decision-cli`.
- Facade re-exports in `crates/decision-cli/src/core/mod.rs` (`pub use dec_graph::…`) so existing `crate::core::graph::…` / `crate::core::store::…` / `crate::core::bundle::…` import paths keep compiling.
- Typed write methods (e.g. the `GraphWriter::write_archetype` planned by [FT-147](FT-147)) now have their declared home in this crate.

### State

- No graph-resident state changes, no store format changes, no SHACL behaviour changes. Source relocation only.

### Behaviour

1. Create the crate and move the listed modules, updating their `use` paths to import domain types from `dec-ontology`.
2. Modules that mix graph access with orchestration logic are split at the seam: the store-touching half moves here, the orchestration half stays for [FT-169](FT-169). Split points recorded in the commit message.
3. Add facades; full workspace compiles with no feature-slice import churn.
4. `cargo test --workspace` green; `product verify --platform` green; `scripts/checks/crate-dependency-topology.sh` now asserts the dec-graph arrows for real.

### Invariants

- `dec-graph` does not depend on `dec-harness`, `decision-cli`, or `clap` (enforced by `scripts/checks/crate-dependency-topology.sh`).
- The GraphWriter chokepoint remains the single SHACL-enforced write path ([ADR-041](ADR-041)) — the move must not open a second write route.
- Existing integration tests pass unmodified.

### Error handling

- No new runtime error surface. Topology violations surface as exit 1 from the topology TC and as Cargo cycle errors.

### Boundaries

- Workers still never touch this crate ([ADR-008](ADR-008)): graph access stays harness-side.
- Dispatch/drive/worker machinery is explicitly not in this crate — that is [FT-169](FT-169).

## Out of scope

- [FT-169](FT-169)'s harness extraction.
- Query or shape content changes, store format changes, performance work.
- Facade burn-down.