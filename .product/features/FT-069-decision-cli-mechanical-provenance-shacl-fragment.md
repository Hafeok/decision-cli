---
id: FT-069
title: 'decision-cli: Mechanical-provenance SHACL fragment (PROV-O wasGeneratedBy / wasAttributedTo / generatedAtTime)'
phase: 3
status: complete
depends-on: []
adrs:
- ADR-038
- ADR-013
- ADR-016
- ADR-004
tests:
- TC-119
domains:
- data-model
domains-acknowledged:
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  data-model: Domain 'data-model' is in scope of this feature; not paving in extra cross-cutting governance beyond the linked ADRs.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
---

## Description

Ship the universal mechanical-provenance SHACL NodeShape that every artifact-type shape composes in via `sh:and`. The fragment encodes PROV-O mechanical provenance — *how* an artifact was physically produced — and is the universal half of the dual-provenance discipline (ADR-038).

Three fields, three semantics:

- `prov:wasGeneratedBy` — the Session that produced this artifact. Single-valued; an artifact has exactly one producing session.
- `prov:wasAttributedTo` — the Agent (role + model, or role + human) the Session was attributed to. Multi-valued allowed when a session is jointly attributed (e.g., human-supervised LLM session).
- `prov:generatedAtTime` — the write timestamp from the GraphWriter transaction. Single-valued, monotonically increasing within a named graph.

These triples are populated by GraphWriter from the session record handed in by the harness's session-completion handler. **Workers do not author them.** That separation is what makes mechanical provenance uniformly trustworthy across the system.

This feature also defines the `:SessionProvenanceShape` extension for Session artifacts themselves, which carry `prov:used` (the bundle), `prov:wasInformedBy` (prior sessions whose outputs informed this one), and `prov:wasAssociatedWith` (the Session's Agent attribution).

## Functional Specification

### Inputs

- The PROV-O ontology IRIs (`prov:wasGeneratedBy`, `prov:wasAttributedTo`, `prov:generatedAtTime`, `prov:used`, `prov:wasInformedBy`, `prov:wasAssociatedWith`).
- The decision-cli ontology IRIs for `:Session` and `:Agent` (already partially defined in `crates/decision-cli/src/core/ontology/` for FT-006).
- The session-record struct produced by the harness's session-completion handler (carries session ID, attributed Agent IRIs, completion timestamp).

### Outputs

- `crates/decision-cli/src/core/ontology/shapes/mechanical-provenance.ttl` — the universal `:MechanicalProvenanceShape` NodeShape.
- The same file (or a sibling) declares `:SessionProvenanceShape` extending the universal shape for Session artifacts.
- Rust constants in `core/ontology/` exposing the shape IRIs so feature code can compose them by name rather than by string.
- `Agent` and `Session` rdf:type declarations in the base ontology (extending FT-006 if not already present).

### State

- Shape files are checked in under `crates/decision-cli/src/core/ontology/shapes/`. They are loaded into the orchestration store at `dec init` time (extends FT-006's bootstrap path).
- No on-disk format changes outside the new TTL files.

### Behaviour

1. Author `shapes/mechanical-provenance.ttl`:

   ```turtle
   @prefix sh:   <http://www.w3.org/ns/shacl#> .
   @prefix prov: <http://www.w3.org/ns/prov#> .
   @prefix xsd:  <http://www.w3.org/2001/XMLSchema#> .
   @prefix dec:  <https://decision-cli.dev/ns#> .

   dec:MechanicalProvenanceShape a sh:NodeShape ;
     sh:property [
       sh:path prov:wasGeneratedBy ;
       sh:minCount 1 ; sh:maxCount 1 ;
       sh:class dec:Session
     ] ;
     sh:property [
       sh:path prov:wasAttributedTo ;
       sh:minCount 1 ;
       sh:class dec:Agent
     ] ;
     sh:property [
       sh:path prov:generatedAtTime ;
       sh:minCount 1 ; sh:maxCount 1 ;
       sh:datatype xsd:dateTime
     ] .

   dec:SessionProvenanceShape a sh:NodeShape ;
     sh:targetClass dec:Session ;
     sh:and ( dec:MechanicalProvenanceShape ) ;
     sh:property [
       sh:path prov:used ;
       sh:nodeKind sh:IRI ;
       sh:class dec:Artifact
     ] ;
     sh:property [
       sh:path prov:wasInformedBy ;
       sh:nodeKind sh:IRI ;
       sh:class dec:Session
     ] ;
     sh:property [
       sh:path prov:wasAssociatedWith ;
       sh:minCount 1 ;
       sh:class dec:Agent
     ] .
   ```

2. Extend `core/ontology/loader.rs` (or the equivalent) to load the new shape file alongside existing ontology files at orchestration-store bootstrap.

3. Expose Rust constants for the IRIs:

   ```rust
   pub const MECHANICAL_PROVENANCE_SHAPE: &str = "https://decision-cli.dev/ns#MechanicalProvenanceShape";
   pub const SESSION_PROVENANCE_SHAPE:    &str = "https://decision-cli.dev/ns#SessionProvenanceShape";
   ```

4. Ensure `dec:Session` and `dec:Agent` rdf:type declarations exist in the base ontology TTL (extend FT-006's `base.ttl` if necessary). `Session` already exists as a class for ADR-004 / PROV-O integration; this feature adds the SHACL shape governing it.

### Invariants

- `:MechanicalProvenanceShape` is a *fragment* shape — it has no `sh:targetClass`. It is only meaningful when composed into a type-specific shape via `sh:and` (FT-072 wires this for every artifact type).
- `prov:wasGeneratedBy` cardinality is exactly 1. An artifact has exactly one producing Session; any apparent re-generation is a *new* artifact (new IRI) per ADR-002 (graph-as-state, no mutation).
- `prov:generatedAtTime` is monotonically increasing within a named graph: GraphWriter assigns the timestamp from its transaction clock, which is single-writer per graph.
- The shape is read-only at orchestration-store runtime. Mutations to the shape require a new ADR + a new version of the shape file; the loader treats the shape set as immutable per bootstrap.

### Error handling

- If the shape file is malformed at bootstrap, `dec init` fails with a `BootstrapError::ShapeLoad` and prints the parser diagnostics. Non-recoverable; operator fixes the file.
- Runtime SHACL validation failures against this fragment are reported by GraphWriter (FT-073), not by this feature.

### Boundaries

- **In scope.** The TTL shape file. The Rust constants for the shape IRIs. The loader-extension wiring. Augmenting `base.ttl` with any missing `:Session` / `:Agent` class declarations.
- **Out of scope.** GraphWriter's enforcement of the shape (FT-073). The motivational vocabulary (FT-070). The per-type composition (FT-072). The Python SDK side's pyshacl loading (FT-072 / FT-073).

## Out of scope

- Per-attribute Agent shape (role + model decomposition). Slice-2+; `dec:Agent` is opaque-class for slice 1.
- `prov:wasRevisionOf` / `prov:wasQuotedFrom` and other PROV relations not part of the mechanical block. Future extensions.
- Hand-authored mechanical provenance from worker code. Workers never author this fragment; the harness does.

## References

- [ADR-038](ADR-038) — Dual-provenance discipline (the framing this fragment implements the universal half of).
- [ADR-004](ADR-004) — PROV-O for events and sessions (the substrate this fragment extends).
- [FT-001](FT-001) — GraphWriter chokepoint (the actor that materializes mechanical triples).
- [FT-006](FT-006) — Embedded base ontology and SHACL shapes (the loader path this feature extends).
- `docs/ddd/Implementing_DDD.md` §3 — PROV-O substrate.
