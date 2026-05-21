---
id: FT-045
title: 'decision-cli: coverage check primitive'
phase: 2
status: complete
depends-on:
- FT-036
adrs:
- ADR-028
- ADR-030
- ADR-031
tests:
- TC-068
- TC-069
domains: []
domains-acknowledged:
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-045 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-045 does not introduce or modify a role catalog entry.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-045 produces no feedback artifacts.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-045 has no feedback to gate.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-045 is out of scope for the pairing.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-045 is a read-only SPARQL primitive that performs no writes.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-045's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-045 produces no feedback artifacts.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-045 does not author or modify a fitness-function artifact.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-045 produces no action/interpretation pair.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-045 neither emits nor consumes verdicts.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-045 has no CLI entry of its own and inherits the resolved working directory from its callers.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-045 neither emits nor routes feedback.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-045's code is organised under that migration as core substrate.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-045 does not cross or alter that boundary.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-045 produces no new Session or event type.
---

## Description

The coverage check primitive — a pure read-only query over the orchestration store that answers two questions:

1. **Per-graph coverage.** Given a feature and a graph, which of the feature's TCs are covered by the graph (i.e. referenced by some step's `dec:providesEvidenceFor`), and which are not?
2. **Feature coverage roll-up.** Given a feature and a set of candidate graphs, are all of the feature's TCs covered by *at least one* graph in the set?

Pure substrate for [ADR-030](ADR-030) and [ADR-031](ADR-031): it is the SPARQL query both ride on. Lives entirely in `core::verify::coverage::*`. No CLI surface in this slice; consumers are [FT-046](FT-046), [FT-047](FT-047), [FT-048](FT-048), and a future `dec verify check` (slice 3).

One subcommand → one slice — except this slice has *no* subcommand. It is a pure primitive whose `pub` surface is two functions and two value types. It is sliced separately because [FT-046](FT-046) and [FT-047](FT-047) both depend on it; if it lived inside either, the other would couple to a feature it has no business knowing about.

## Functional Specification

### Inputs

- A feature id (`FT-NNN`) — resolved against the product-cli artifact store; product-cli's read-only feature resolution is already available to `core` via the existing context-bundle integration (see `features/implement/bundle.rs` for the established pattern).
- An optional candidate set of `VerificationGraph` ids (`Vec<GraphId>`). When absent, all graphs in the store whose `dec:verifies` matches the feature, any of its TCs, or its parent-of-tests are considered candidates.
- The orchestration store handle ([FT-036](FT-036)'s named-graph projection).

### Outputs

- `CoverageReport`:
  ```rust
  pub struct CoverageReport {
      pub feature: FeatureId,
      pub all_tcs: Vec<TcId>,            // every TC the feature lists
      pub covered: Vec<CoverageHit>,     // (tc, graph_id, step_id) for each covering step
      pub uncovered: Vec<TcId>,          // TCs with zero covering steps in candidates
      pub considered: Vec<GraphId>,      // which graphs were examined
  }
  pub struct CoverageHit { pub tc: TcId, pub graph: GraphId, pub step: StepId }
  ```
- `fn feature_covered_by(feature: FeatureId, graph: GraphId, store: &Store) -> Result<CoverageReport>`
- `fn feature_coverage(feature: FeatureId, candidates: Option<Vec<GraphId>>, store: &Store) -> Result<CoverageReport>`
- A `pub` SPARQL query string in `core::verify::coverage::queries::COVERAGE` for downstream reuse / debugging.

### State

- None. Read-only over the existing store; the primitive does not write quads or files.

### Behaviour

1. Resolve the feature's TCs via product-cli's existing resolution (`features/implement/bundle.rs` shows the pattern); the result is `Vec<TcId>`.
2. If a candidate set is given, validate each id exists in the verify-graph named graph (`Error::ArtifactNotFound` on miss). If absent, query the store for every `dec:VerificationGraph` whose `dec:verifies` is the feature or any of its TCs.
3. For each `(tc, graph)` pair, run the SPARQL CONSTRUCT:
   ```sparql
   PREFIX dec: <https://decision-cli.dev/ns/>
   SELECT ?step WHERE {
     GRAPH <https://decision-cli.dev/ns/graph/verify-graph> {
       ?graph dec:steps/rdf:rest*/rdf:first ?step .
       ?step dec:providesEvidenceFor ?tc .
     }
   }
   ```
   Bind `?graph` and `?tc`; collect matching `?step`s.
4. Aggregate hits into `CoverageReport.covered`; TCs with zero hits across all considered graphs go into `uncovered`.
5. Return the report; never mutate state.

### Invariants

- The primitive is **side-effect-free** — no writes, no logging beyond a single trace span, no PROV-O activity.
- The result is deterministic for a fixed store snapshot.
- The primitive does not interpret TC bodies — coverage is purely structural via `dec:providesEvidenceFor`.
- The primitive accepts an empty candidate set without erroring (returns `uncovered = all_tcs`, `covered = []`).
- A feature with zero TCs returns `covered = uncovered = []` and is treated by callers as covered (the chain gate's policy decision lives in [FT-047](FT-047), not here).

### Error handling

- Unknown feature id → `Error::ArtifactNotFound { kind: "Feature", id }`.
- Unknown candidate graph id → `Error::ArtifactNotFound { kind: "VerificationGraph", id }`.
- Store unreachable → `Error::StoreUnreachable`.

### Boundaries

- **In scope.** Two pure functions, the `CoverageReport`/`CoverageHit` types, the SPARQL query, integration tests against an in-memory store.
- **Out of scope.** CLI surface (slice 3's `dec verify check` consumes this). The chain-integrity dispatch policy ([FT-047](FT-047)). Cross-environment coverage roll-up (slice 3+). Per-TC weighting / partial-credit coverage (slice 3+).

## Out of scope

- CLI / MCP surface in slice 2.6.
- Persisting coverage reports as artifacts.
- Free-text matching of step bodies against TC bodies.
- Weighted or partial coverage models.
- Time-windowed coverage (e.g. "last 30 days").
