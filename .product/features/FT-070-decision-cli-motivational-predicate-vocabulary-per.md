---
id: FT-070
title: 'decision-cli: Motivational-predicate vocabulary per artifact type'
phase: 3
status: complete
depends-on: []
adrs:
- ADR-038
- ADR-039
- ADR-013
- ADR-016
tests:
- TC-120
domains:
- data-model
domains-acknowledged:
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  data-model: Domain 'data-model' is in scope of this feature; not paving in extra cross-cutting governance beyond the linked ADRs.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
---

## Description

Catalog the slice-1 motivational-predicate vocabulary — the per-artifact-type controlled list of predicates that satisfy the motivational half of the dual-provenance discipline (ADR-038). Each artifact type's SHACL shape (FT-072) requires *at least one* of its listed predicates via `sh:or` composition; range constraints (the target class of each predicate) are encoded per predicate; all motivational predicates are declared as `rdfs:subPropertyOf prov:wasDerivedFrom` (ADR-039) so the full-chain query (FT-075) walks them uniformly.

The slice-1 vocabulary covers existing product-cli artifact types plus the proposed types that immediate-neighbour Briefs (`brief:pipeline-worker-slice-1`, `brief:worker-distribution-slice-1`) introduce. It is expected to grow (Brief's `ack:vocabulary-will-grow`); growth is governed by individual ADRs per addition.

## Functional Specification

### Inputs

- The decision-cli ontology base namespace (`https://decision-cli.dev/ns#`).
- PROV-O (`prov:wasDerivedFrom` as the parent property).
- The existing artifact types in product-cli (Feature, ADR, TC, Dependency) and the proposed types from neighbour Briefs (Brief, Acknowledgement, Feedback, DiscoveryFinding, Question, WorkerImage, ConformanceAudit, Model, Policy, WorkerImageSubmission, Subscription, Dispatch).

### Outputs

- `crates/decision-cli/src/core/ontology/shapes/motivational-predicates.ttl` — central predicate declarations (one `rdf:Property` per predicate, with `rdfs:subPropertyOf prov:wasDerivedFrom`, `rdfs:domain`, `rdfs:range`).
- A reference table in this feature_spec body (below) enumerating the per-type alternatives. This table is the single source of truth that the per-type shape files (FT-072) implement.
- Documentation in `docs/ddd/` (or under `core/ontology/`) explaining the vocabulary, its growth policy, and how to propose additions.

### State

The vocabulary is loaded into the orchestration store at `dec init` (alongside FT-069's mechanical fragment, via the FT-006 loader extension). No per-instance state.

### Behaviour

1. Author `shapes/motivational-predicates.ttl`. For every predicate in the table below, emit:

   ```turtle
   dec:addresses
       a rdf:Property ;
       rdfs:subPropertyOf prov:wasDerivedFrom ;
       rdfs:domain dec:Artifact ;                # broad; per-type shapes tighten
       rdfs:range  dec:Feedback ;                # the allowable target class for THIS predicate
       rdfs:label "addresses" ;
       rdfs:comment "The artifact exists to resolve the named Feedback." .
   ```

   For predicates with multiple legitimate ranges (e.g. `:responds_to` accepts `Request`, `Feedback`, or `Question` depending on context), use `owl:unionOf` on `rdfs:range` or omit the range constraint here and let the per-type shape pin it.

2. The per-type alternatives are *not* declared in this file — they are declared per-type in FT-072's shape files. This file holds only the predicate declarations.

3. Slice-1 vocabulary (the table the shape files implement):

   | Artifact type      | Required (at least one of)                                                                                                  |
   |--------------------|------------------------------------------------------------------------------------------------------------------------------|
   | Feature            | `addresses → Feedback` \| `decomposes_from → Brief` \| `originated_from → DiscoveryFinding` \| `responds_to → Question`     |
   | ADR                | `decides_for → Feature` \| `addresses → Question` \| `supersedes → ADR`                                                      |
   | TC (TestCriterion) | `validates → Feature` \| `validates → ADR`                                                                                   |
   | Dependency         | `required_by → Feature` \| `required_by → ADR`                                                                               |
   | Brief              | `responds_to → Request` \| `responds_to → Feedback` \| boundary-artifact class membership                                    |
   | Acknowledgement    | `motivated_by → Brief` \| `motivated_by → ADR`                                                                               |
   | Feedback           | `observed_in → Session` \| `observed_via → SensingAction` \| `produced_by → Role`                                            |
   | DiscoveryFinding   | `derived_from → SensingAction`                                                                                               |
   | Question           | `raised_in → Session` \| `raised_by → Brief` \| `raised_by → ADR`                                                            |
   | WorkerImage        | `addresses → Feedback` \| `decomposes_from → Brief` \| `originated_from → DiscoveryFinding`                                  |
   | ConformanceAudit   | `audits → WorkerImage`                                                                                                       |
   | Model              | `decomposes_from → Brief` \| `addresses → CapabilityGap`                                                                     |
   | Policy             | `decomposes_from → Brief` \| `addresses → Feedback`                                                                          |
   | WorkerImageSubmission | boundary-artifact class membership                                                                                       |
   | Session            | (carries its own mechanical block via SessionProvenanceShape; not a derived artifact)                                       |
   | Subscription       | `motivated_by → Brief` \| `motivated_by → Policy`                                                                            |
   | Dispatch           | (carries its own mechanical block; dispatches are session-generated artifacts)                                              |
   | QueryTemplate      | `decomposes_from → Brief` \| `addresses → Feedback` \| boundary-artifact class membership                                    |

   Notes on selected entries:
   - **Feature** picks up `responds_to → Question` for the case where a Feature resolves an OpenQuestion raised in an earlier session. This is the path open questions take from narrative to first-class artifact.
   - **Brief** can satisfy the requirement via boundary-artifact class membership (ADR-040): a Brief that originates from outside the orchestration graph has no graph-internal motivational origin.
   - **ADR's `supersedes → ADR`** captures the case where one ADR's existence is motivated by revising another. The full-chain query walks back through the supersession chain.
   - **WorkerImageSubmission** is always a boundary artifact (entering from CI).
   - **Session** does not appear in the motivational table because Sessions are not "derived from" upstream artifacts in the motivational sense; they reference their bundle via `prov:used` (mechanical). The Session is the Activity that connects the two flavors.

### Invariants

- Every predicate in the vocabulary is declared `rdfs:subPropertyOf prov:wasDerivedFrom` so generic traversal via `prov:wasDerivedFrom*` is complete (ADR-039).
- Range constraints declared on each predicate must match what the per-type shape files (FT-072) enforce. A CI fitness check verifies this: load the predicate declarations and the type-shape files; for every per-type `sh:property [ sh:path :foo ; sh:class :Bar ]`, assert that `dec:foo rdfs:range dec:Bar` (or that the per-type shape narrows from `unionOf` to `Bar`).
- Adding a new predicate requires an ADR. The ADR documents which type(s) the predicate is added to, what range constraints apply, and what gap it fills. Vocabulary growth is governed work, not ad-hoc.
- Removing a predicate is a *breaking* change requiring an ADR + migration: existing artifacts using the predicate must be re-authored or have their edges rewritten through the standard governance flow.

### Error handling

- Malformed predicate declarations at bootstrap → `BootstrapError::ShapeLoad`. Non-recoverable.
- Per-type shape files (FT-072) referencing a predicate not declared here → SHACL load-time error reported by GraphWriter init; operator fixes.
- The CI fitness check (range-agreement) failing → build break, with the offending predicate and per-type shape named.

### Boundaries

- **In scope.** The `motivational-predicates.ttl` file with one declaration per predicate. The slice-1 vocabulary table (canonical here in the feature_spec body). Documentation of growth policy. The build-time range-agreement fitness check.
- **Out of scope.** Per-type `sh:or` composition (FT-072 — one shape file per artifact type that imports this vocabulary). Predicate versioning machinery (`feature:predicate-vocabulary-versioning`, slice 2+ per the Brief's excludes). The migration of existing artifacts' informal edges to declared motivational edges (FT-074).

## Out of scope

- New artifact types beyond the slice-1 catalog. Each new type ships with its own ADR-and-feature pair under the framework's normal process.
- Reasoning behaviour on `rdfs:range` constraints across subclass hierarchies (open question 2 from the Brief — defer to FT-073 implementation).
- Cross-system motivational references (open question 3 — opaque URIs resolved at audit time; not implemented in slice 1).

## References

- [ADR-038](ADR-038) — Dual-provenance discipline (the framing this vocabulary implements the per-type half of).
- [ADR-039](ADR-039) — Motivational predicates as `subPropertyOf prov:wasDerivedFrom` (the reason every entry here carries the subProperty annotation).
- [ADR-040](ADR-040) — BoundaryArtifact class (the alternative to motivational edges for boundary-originated instances).
- [FT-069](FT-069) — Mechanical provenance fragment (the universal half; composed into the same per-type shapes).
- [FT-072](FT-072) — Per-type shape files that consume this vocabulary.
