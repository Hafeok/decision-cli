---
id: FT-071
title: 'decision-cli: BoundaryArtifact class and external_origin field'
phase: 3
status: complete
depends-on: []
adrs:
- ADR-040
- ADR-013
- ADR-016
tests:
- TC-121
domains:
- data-model
domains-acknowledged:
  data-model: Domain 'data-model' is in scope of this feature; not paving in extra cross-cutting governance beyond the linked ADRs.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
---

## Description

Define the `:BoundaryArtifact` class and its `:external_origin` field — the orphan-motivational escape hatch from ADR-040. A BoundaryArtifact is an artifact whose motivational origin is legitimately external to the orchestration graph (sensing-action outputs, initial-request artifacts, bootstrap-era writes, migration backfills). Instances are exempt from the motivational-provenance `sh:or` requirement at the type-shape level but must still carry the mechanical block (FT-069) and an explicit `:external_origin` string.

This feature ships the class declaration, the shape that enforces `:external_origin`, and the subclass declarations for the four slice-1 boundary kinds (`SensingActionOutput`, `InitialRequest`, `BootstrapArtifact`, `MigrationBackfill`). The per-type shapes (FT-072) reference `BoundaryArtifact` membership in the first branch of their `sh:or` block.

## Functional Specification

### Inputs

- The decision-cli ontology base namespace (`https://decision-cli.dev/ns#`).
- The SHACL namespace and the existing ontology loader (FT-006).
- The mechanical-provenance fragment from FT-069 (composed into the BoundaryArtifact shape, since boundary artifacts still carry mechanical provenance).

### Outputs

- `crates/decision-cli/src/core/ontology/shapes/boundary-artifact.ttl` — defines the `BoundaryArtifact` class, its `BoundaryArtifactShape`, and the four slice-1 subclasses with their per-subclass shape extensions.
- Rust constants for the IRIs in `core/ontology/`.
- Loader-extension wiring (alongside FT-069's mechanical fragment).

### State

Loaded into the orchestration store at `dec init`. No per-instance state introduced by this feature beyond the type and shape declarations.

### Behaviour

1. Author `shapes/boundary-artifact.ttl`:

   ```turtle
   @prefix rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
   @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
   @prefix sh:   <http://www.w3.org/ns/shacl#> .
   @prefix xsd:  <http://www.w3.org/2001/XMLSchema#> .
   @prefix dec:  <https://decision-cli.dev/ns#> .

   # Top class
   dec:BoundaryArtifact a rdfs:Class ;
       rdfs:label "BoundaryArtifact" ;
       rdfs:comment "Artifact whose motivational origin is external to the orchestration graph." .

   # Subclasses (open question 4 resolution: subclasses with per-subtype shape extensions)
   dec:SensingActionOutput a rdfs:Class ;
       rdfs:subClassOf dec:BoundaryArtifact ;
       rdfs:label "SensingActionOutput" .

   dec:InitialRequest a rdfs:Class ;
       rdfs:subClassOf dec:BoundaryArtifact ;
       rdfs:label "InitialRequest" .

   dec:BootstrapArtifact a rdfs:Class ;
       rdfs:subClassOf dec:BoundaryArtifact ;
       rdfs:label "BootstrapArtifact" .

   dec:MigrationBackfill a rdfs:Class ;
       rdfs:subClassOf dec:BoundaryArtifact ;
       rdfs:label "MigrationBackfill" .

   # Base shape: mechanical block + external_origin
   dec:BoundaryArtifactShape a sh:NodeShape ;
       sh:targetClass dec:BoundaryArtifact ;
       sh:and ( dec:MechanicalProvenanceShape ) ;
       sh:property [
           sh:path dec:external_origin ;
           sh:minCount 1 ; sh:maxCount 1 ;
           sh:datatype xsd:string ;
           sh:minLength 1
       ] .

   # MigrationBackfill must additionally carry the synthetic-annotation flag
   dec:MigrationBackfillShape a sh:NodeShape ;
       sh:targetClass dec:MigrationBackfill ;
       sh:property [
           sh:path dec:isMigrationBackfill ;
           sh:hasValue true ;
           sh:minCount 1 ; sh:maxCount 1
       ] .
   ```

2. Extend the ontology loader (FT-006) to load this file at bootstrap, alongside FT-069's mechanical fragment.

3. Expose Rust constants:

   ```rust
   pub const BOUNDARY_ARTIFACT_CLASS: &str   = "https://decision-cli.dev/ns#BoundaryArtifact";
   pub const SENSING_ACTION_OUTPUT: &str     = "https://decision-cli.dev/ns#SensingActionOutput";
   pub const INITIAL_REQUEST: &str           = "https://decision-cli.dev/ns#InitialRequest";
   pub const BOOTSTRAP_ARTIFACT: &str        = "https://decision-cli.dev/ns#BootstrapArtifact";
   pub const MIGRATION_BACKFILL: &str        = "https://decision-cli.dev/ns#MigrationBackfill";
   pub const EXTERNAL_ORIGIN_PROP: &str      = "https://decision-cli.dev/ns#external_origin";
   ```

4. Per-type shapes (FT-072) consume the class by including the boundary branch as the first alternative in their `sh:or`:

   ```turtle
   sh:or (
       [ a sh:NodeShape ; sh:class dec:BoundaryArtifact ]
       # … motivational alternatives …
   )
   ```

### Invariants

- A BoundaryArtifact carries the mechanical block (universal — boundary artifacts still pass through a Session, even if synthetic).
- `:external_origin` is required, single-valued, non-empty string. The string format is unstructured in slice 1; per-subtype tightening is deferred to slice 2+.
- `MigrationBackfill` instances must additionally carry `:isMigrationBackfill true`, enforced by `MigrationBackfillShape`. Required by ADR-042 so synthetic provenance is queryable.
- Subclass relationships use `rdfs:subClassOf`, so a SHACL `sh:class dec:BoundaryArtifact` constraint accepts any subclass instance (subClassOf reasoning is the default for `sh:class` in oxigraph-shacl per the open-question-2 resolution in FT-073).
- Class membership and motivational-predicate edges are not mutually exclusive — an artifact may legitimately be both a BoundaryArtifact and carry a `:decomposes_from` edge (e.g. a migration-backfilled artifact whose informal pre-discipline `adrs:` front-matter mapped to a `:decides_for` motivational edge). The `sh:or` is permissive: either alternative is sufficient.

### Error handling

- Malformed TTL at bootstrap → `BootstrapError::ShapeLoad`.
- A BoundaryArtifact written without `:external_origin` → SHACL violation at GraphWriter (FT-073) with property path `:external_origin`.
- A MigrationBackfill written without `:isMigrationBackfill true` → SHACL violation at GraphWriter; migration tooling (FT-074) is the only legitimate producer of these and must set the flag.

### Boundaries

- **In scope.** The class + four subclasses + their shapes + `:external_origin` field. Rust constants. Loader wiring.
- **Out of scope.** Per-type composition of the boundary branch into `sh:or` blocks (FT-072). Migration tooling that produces `MigrationBackfill` instances (FT-074). Per-subtype format tightening of `:external_origin` (deferred). Federation-time resolution of cross-system boundary references (Brief open question 3, deferred).

## Out of scope

- Additional BoundaryArtifact subclasses beyond the slice-1 four. Each new subclass ships under its own feature with an ADR justifying the new boundary kind.
- Structured `:external_origin` (e.g. typed by subtype: `ChatTranscriptId` for InitialRequest Briefs, `CIRunRef` for WorkerImageSubmissions, `SensingActionRef` for SensingActionOutputs). Slice 2+.

## References

- [ADR-040](ADR-040) — BoundaryArtifact as the orphan-motivational escape hatch (the decision this feature implements).
- [ADR-038](ADR-038) — Dual-provenance discipline (the framing).
- [FT-069](FT-069) — Mechanical-provenance fragment (composed into BoundaryArtifactShape).
- [FT-072](FT-072) — Per-type shape files that reference BoundaryArtifact class membership.
- [FT-074](FT-074) — Migration (legitimate producer of `MigrationBackfill` instances).
