---
id: ADR-040
title: BoundaryArtifact class as the orphan-motivational escape hatch
status: proposed
features:
- FT-071
supersedes: []
superseded-by: []
domains:
- data-model
scope: cross-cutting
---

## Context

ADR-038 requires every artifact to declare at least one motivational-provenance edge. The system has artifacts that legitimately have *no graph-internal motivational ancestor*:

1. **Sensing-action outputs.** Artifacts produced by monitoring reads, log queries, API polls, user interviews, market signals. The motivational origin is *external reality*; the mechanical provenance traces back through the sensing action's Session, but the chain terminates at the system boundary, not at another graph node.
2. **Initial-request artifacts.** Artifacts entering from outside the orchestration graph: customer requests, an upstream system's Feedback, a CI-posted WorkerImageSubmission, a Brief authored before the system existed (the `brief:dual-provenance-discipline` Brief that motivated this entire family of features is the canonical example).
3. **Bootstrap artifacts.** The catalog, the initial shape files, the first Role declarations — written into the graph before any Session existed (see also FT-074's `:BootstrapSession` synthetic).
4. **Migration backfills.** Artifacts whose informal pre-discipline provenance was reconstructed by tooling (FT-074).

Forcing these to invent synthetic motivational ancestors degrades the discipline from "every artifact has real origin" to "every artifact has at least a placeholder."

## Decision

**Introduce an explicit `:BoundaryArtifact` class. SHACL shapes for artifact types accept `BoundaryArtifact` class membership as a satisfying alternative for the motivational requirement, via the first branch of each type's `sh:or`.**

Per-type shape pattern:

```turtle
:FeatureShape a sh:NodeShape ;
  sh:targetClass :Feature ;
  sh:and ( :MechanicalProvenanceShape ) ;
  sh:or (
    [ a sh:NodeShape ; sh:class :BoundaryArtifact ]                                  # boundary escape hatch
    [ sh:property [ sh:path :addresses ;        sh:minCount 1 ; sh:class :Feedback         ] ]
    [ sh:property [ sh:path :decomposesFrom ;   sh:minCount 1 ; sh:class :Brief            ] ]
    [ sh:property [ sh:path :originatedFrom ;   sh:minCount 1 ; sh:class :DiscoveryFinding ] ]
    [ sh:property [ sh:path :respondsTo ;       sh:minCount 1 ; sh:class :Question         ] ]
  ) .
```

A BoundaryArtifact still carries:

- **Mechanical provenance** (the universal block — boundary artifacts are produced by *some* session, even if it is a synthetic `:BootstrapSession`, a sensing-action session, or a `:HistoricalSession`).
- **`:external_origin` field** — a single required string that documents how the artifact entered the system (e.g. the chat transcript ID for a Brief, the CI run ID for a WorkerImageSubmission, the sensing-action ID + external endpoint for a monitoring output). This is not motivational provenance in the graph sense — it does not traverse to another graph node — but it preserves auditability of how the artifact came to exist.

```turtle
:BoundaryArtifactShape a sh:NodeShape ;
  sh:targetClass :BoundaryArtifact ;
  sh:property [
    sh:path :external_origin ;
    sh:minCount 1 ; sh:maxCount 1 ;
    sh:datatype xsd:string
  ] .
```

### Subclasses

Open question 4 in the Brief asks whether `SensingActionOutput`, `InitialRequest`, `MigrationBackfill`, etc. should be subclasses of `BoundaryArtifact` or peer classes. **Decision: subclasses with shape extensions per subtype.** Centralizes the boundary concept and the `:external_origin` requirement; lets each subtype tighten constraints (e.g. `MigrationBackfill` requires an `:isMigrationBackfill true` annotation per ADR-042) without re-declaring the base shape.

### Alternatives considered

- **Don't allow orphans; require synthetic motivational edges.** Forces authors to invent fake ancestors. Discipline degrades. Rejected.
- **Allow type-specific orphan handling without a class.** Each artifact type defines its own "no motivational required" condition. Works, but scatters the boundary concept across many shapes. Rejected for uniformity.
- **Class membership (adopted).** Centralizes the boundary concept; uniform across types; explicit `external_origin` field preserves auditability at the boundary.

## Consequences

**Positive.**

- The discipline holds at the boundary; it just acknowledges that the boundary exists. Every artifact still has *some* declared origin, even if that origin is external.
- Audit traversal terminates cleanly at BoundaryArtifact nodes — the full-chain query (FT-075 / ADR-043) uses the `BoundaryArtifact` class as a terminal condition rather than walking forever on a missing edge.
- Sensing-action outputs and initial requests are first-class instead of being smuggled in as ad-hoc records that don't conform to the discipline.

**Negative / accepted costs.**

- Authors of new artifact types must remember to include the `BoundaryArtifact` class-membership branch in their `sh:or` if instances of that type can legitimately originate at the boundary. Forgetting it means boundary instances of that type fail validation. Mitigation: the type-shape template in FT-072's `shapes/` directory includes the boundary branch as a comment-marked default that authors can opt out of.
- The `:external_origin` string is unstructured. A future tightening might introduce per-subtype shapes for it (chat-transcript ID format, CI-run ID format, sensing-action reference, etc.) — deferred to slice 2+.

**Boundary enforcement.** SHACL `sh:class` checks are validator-enforced. An artifact declared as `:Feature` that lacks both motivational edges *and* `:BoundaryArtifact` class membership fails validation at GraphWriter (ADR-041).

## Relationship to existing ADRs

- **ADR-038 (dual provenance).** This ADR is the orphan-handling clause of the discipline ADR-038 establishes.
- **ADR-022 (Feedback as a first-class flow class).** Compatible — Feedback originating from a sensing action is a BoundaryArtifact subtype; Feedback originating from a Session has a normal motivational chain.

## Status

Proposed. Implementation in FT-071. The Brief and all initial-request artifacts (the three slice-1 Briefs in the Brief family) will declare BoundaryArtifact class membership once the type lands and the migration runs.
