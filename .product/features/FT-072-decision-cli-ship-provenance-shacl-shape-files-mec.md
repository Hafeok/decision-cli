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
tests:
- TC-122
domains:
- data-model
domains-acknowledged: {}
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
