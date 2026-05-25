---
id: FT-069
title: 'decision-cli: Mechanical-provenance SHACL fragment (PROV-O wasGeneratedBy / wasAttributedTo / generatedAtTime)'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-038
tests:
- TC-119
domains:
- data-model
domains-acknowledged: {}
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
