---
id: FT-072
title: 'decision-cli: Ship provenance SHACL shape files (mechanical, motivational, boundary, per-type)'
phase: 3
status: planned
depends-on:
- FT-069
- FT-070
- FT-071
adrs:
- ADR-041
- ADR-013
- ADR-016
tests:
- TC-122
domains:
- data-model
domains-acknowledged:
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  data-model: Domain 'data-model' is in scope of this feature; not paving in extra cross-cutting governance beyond the linked ADRs.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
---

## Description

Ship the SHACL shape files that materialise the dual-provenance discipline (ADR-038): the universal mechanical block (FT-069), the boundary-artifact class (FT-071), the motivational predicate vocabulary (FT-070), and one shape file per artifact type that composes them via `sh:and` (mechanical) + `sh:or` (motivational + boundary).

The shape files are the single source of truth consumed by:
- GraphWriter's authoritative validator (FT-073, oxigraph-shacl in Rust).
- The Python SDK's defensive validator (pyshacl side, in workers).
- product-cli's existing audit infrastructure.
- Any future system that joins the platform.

One source per shape; everything else consumes from there.

## Functional Specification

### Inputs

- The mechanical-provenance fragment (FT-069 — `shapes/mechanical-provenance.ttl`).
- The boundary-artifact class and shape (FT-071 — `shapes/boundary-artifact.ttl`).
- The motivational-predicate vocabulary (FT-070 — `shapes/motivational-predicates.ttl`).
- The slice-1 artifact-type catalog (Feature, ADR, TC, Dependency, Brief, Acknowledgement, Feedback, DiscoveryFinding, Question, WorkerImage, ConformanceAudit, Model, Policy, WorkerImageSubmission, Subscription, Dispatch, QueryTemplate — from FT-070's table).

### Outputs

Files under `crates/decision-cli/src/core/ontology/shapes/`:

- `mechanical-provenance.ttl` (from FT-069)
- `motivational-predicates.ttl` (from FT-070)
- `boundary-artifact.ttl` (from FT-071)
- `feature.ttl` — `:FeatureShape` composing the mechanical fragment + boundary/motivational alternatives.
- `adr.ttl`
- `tc.ttl`
- `dependency.ttl`
- `brief.ttl`
- `acknowledgement.ttl`
- `feedback.ttl`
- `discovery-finding.ttl`
- `question.ttl`
- `worker-image.ttl`
- `conformance-audit.ttl`
- `model.ttl`
- `policy.ttl`
- `worker-image-submission.ttl`
- `subscription.ttl`
- `dispatch.ttl`
- `query-template.ttl`

And a manifest:

- `shapes/manifest.ttl` — lists all shape files in load order (mechanical and boundary first, then motivational predicates, then per-type), consumed by the ontology loader.

### State

Shape files are checked-in source. They are loaded into the orchestration store at `dec init` (extending FT-006's bootstrap path) and are also packaged into the Python worker SDK so pyshacl can load identical content on the worker side.

### Behaviour

1. **Per-type shape template.** Each type's shape follows the canonical pattern:

   ```turtle
   dec:FeatureShape a sh:NodeShape ;
       sh:targetClass dec:Feature ;
       sh:and ( dec:MechanicalProvenanceShape ) ;
       sh:or (
           [ a sh:NodeShape ; sh:class dec:BoundaryArtifact ]
           [ sh:property [ sh:path dec:addresses        ; sh:minCount 1 ; sh:class dec:Feedback         ] ]
           [ sh:property [ sh:path dec:decomposesFrom   ; sh:minCount 1 ; sh:class dec:Brief            ] ]
           [ sh:property [ sh:path dec:originatedFrom   ; sh:minCount 1 ; sh:class dec:DiscoveryFinding ] ]
           [ sh:property [ sh:path dec:respondsTo       ; sh:minCount 1 ; sh:class dec:Question         ] ]
       ) .
   ```

   Add type-specific property constraints (existing FT-006-era shapes for required title, status, etc.) below the dual-provenance composition. Existing constraints are preserved; the new constraints layer on top.

2. **Load order.** The manifest declares loading sequence: mechanical-provenance, boundary-artifact, motivational-predicates, then per-type. SHACL `sh:and` / `sh:class` references resolve correctly because the referenced shapes/classes are already in the store by the time a per-type shape references them.

3. **Cross-side packaging.** The same TTL files are referenced by:
   - Rust: `core/ontology/loader.rs` reads them via `include_str!` for the embedded slice-1 distribution (per ADR-007).
   - Python SDK: build step copies the directory into `workers/_shared/shapes/` so pyshacl loads identical files.
   - Build-time fitness check: a CI step verifies the Python copy is byte-identical to the Rust source. Divergence is a build break.

4. **The cross-cutting fitness check** (range-agreement) from FT-070 runs against the assembled set: for every per-type `sh:property [ sh:path :foo ; sh:class :Bar ]`, assert `dec:foo rdfs:range dec:Bar` (or that `Bar ∈ unionOf(:foo's range)`).

### Invariants

- Every per-type shape composes `dec:MechanicalProvenanceShape` via `sh:and`. Missing it is a CI fitness-check failure.
- Every per-type shape's `sh:or` first branch is `[ a sh:NodeShape ; sh:class dec:BoundaryArtifact ]` *unless* the type cannot legitimately originate at the boundary (Session, Dispatch — these carry their own mechanical block and are never boundary-originated; their shapes do not include the boundary branch).
- Shape files are immutable at orchestration-store runtime. Updates ship as a new version of the file with an ADR documenting the change. No live editing.
- Python and Rust copies of the shape directory are byte-identical. CI enforces.
- Subclass-aware `sh:class` reasoning is enabled in the validator (resolves open question 2 from the Brief): `sh:class dec:Feedback` accepts subclass instances. Validator config (FT-073) sets this; shape files rely on it.

### Error handling

- Malformed TTL or unresolved class reference at bootstrap → `BootstrapError::ShapeLoad`. Non-recoverable.
- Build-time copy-divergence (Rust vs Python) → CI fails with a diff.
- Build-time range-agreement check fails → CI fails with the offending predicate and per-type shape.

### Boundaries

- **In scope.** All shape files listed above. The manifest. The Rust loader extension. The build-step copy into the worker SDK. The two build-time fitness checks (copy-identical, range-agreement).
- **Out of scope.** Validator runtime behaviour and rejection emission (FT-073). Migration of existing artifacts to conformance (FT-074). The full-chain query (FT-075).

## Out of scope

- Subclass-shape extensions for fine-grained BoundaryArtifact subtypes beyond the slice-1 four (deferred to slice 2+).
- Federation of shape files across systems (worker-distribution Brief; later slice).
- Hot-reload of shape files at runtime. Shape changes require a `dec init` re-bootstrap or equivalent.

## References

- [ADR-038](ADR-038) — Dual provenance.
- [ADR-040](ADR-040) — BoundaryArtifact escape hatch.
- [ADR-041](ADR-041) — SHACL as enforcement mechanism (the validator that consumes these files).
- [FT-069](FT-069), [FT-070](FT-070), [FT-071](FT-071) — Source fragments composed in.
- [FT-006](FT-006) — Embedded base ontology (the loader path this feature extends).
- [ADR-007](ADR-007) — Embedded distribution model (shape files ship inside the binary).
