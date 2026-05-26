---
id: FT-026
title: 'decision-cli: Feedback artifact type and SHACL shape'
phase: 2
status: complete
depends-on:
- FT-001
- FT-006
adrs:
- ADR-022
tests:
- TC-040
domains: []
domains-acknowledged:
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-026 does not introduce or modify a role catalog entry.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-026 runs after the working directory is resolved and does not re-discover it.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-026 neither emits nor consumes verdicts.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-026 does not author or modify a fitness-function artifact.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-026's code is reorganised under that migration, not by this feature.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-026 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-026 produces no new Session or event type and inherits lineage from the harness.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-026 has no feedback to gate.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-026 is out of scope for the pairing.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-026 produces no feedback artifacts.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-026 produces no feedback artifacts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-026 produces no action/interpretation pair.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-026 does not cross or alter that boundary.
---

## Description

Land the `dec:Feedback` artifact type — schema, SHACL shape, write-side validation, read API. Together with [FT-027](FT-027) (lifecycle state machine) and [FT-028](FT-028) (class vocabulary), this is the schema substrate that every other slice-3 feature depends on.

Pure schema-shaped feature: no subscriptions, no workers, no CLI. Lives entirely in `core/`.

## Functional Specification

### Inputs

- The embedded ontology ([FT-006](FT-006)) — extended here.
- The `StreamWriter` chokepoint ([ADR-005](ADR-005)) — extended to recognize `Feedback`.
- The vocab module (`core::vocab`).

### Outputs

- Ontology extension with class `dec:Feedback` and predicates: `dec:feedbackClass` (see [FT-028](FT-028)), `dec:severity`, `dec:targetRole`, `dec:evidence`, `dec:recommendation`, `dec:lifecycleState` (see [FT-027](FT-027)), `dec:sourceSession`, `dec:sourceArtifact`, `dec:addressingArtifact`, `dec:closedBy`, `dec:rejectionReason`, `dec:supersededBy`, `dec:routedAt`, `dec:receivingSession`, `dec:dispositionOverride`, `dec:dispositionRationale`, `dec:inStream`.
- SHACL shape `dec:FeedbackShape` enforcing required fields per [ADR-022](ADR-022). Class-specific and state-specific constraints land in [FT-027](FT-027) and [FT-028](FT-028).
- Rust types under `core::feedback::artifact`:
  - `struct Feedback { class: FeedbackClass, severity: Severity, target_role: RoleId, evidence: String, recommendation: Option<String>, lifecycle_state: LifecycleState, source_session: SessionIri, source_artifact: Option<ArtifactIri>, addressing_artifact: Option<ArtifactIri>, in_stream: StreamIri, … }`
  - `fn to_quads(&self, graph: NamedNodeRef) -> Vec<Quad>` (RDF serialiser).
  - `fn from_quads(store: &Store, iri: &NamedNode) -> Result<Feedback, _>` (read-side reconstructor).
- `StreamWriter` recognises `Feedback` mutations and runs SHACL pre-commit. Failures abort.
- Read API in `core::feedback::read`:
  - `list_open(store, stream) -> Vec<Feedback>` (state ∉ {closed, rejected, superseded}).
  - `list_by_class(store, class) -> Vec<Feedback>`.
  - `list_by_target(store, role_id) -> Vec<Feedback>`.
  - `get(store, iri) -> Result<Feedback, _>`.

### State

- Persistent: each emitted feedback is its own artifact graph in the store, attached via PROV-O to its source session and (when applicable) source artifact, and via `dec:addressingArtifact` to its resolution.

### Behaviour

1. Extend the ontology Turtle in `core/ontology/feedback.ttl` (or in-place in the existing ontology bundle, matching the FT-006 organisation).
2. Add vocab IRIs.
3. Add the Rust struct, `to_quads`, `from_quads`.
4. Extend `StreamWriter` to recognise `Feedback` mutations.
5. Add the read helpers in `core::feedback::read`.
6. Per the slice-level SDP, this module is `core::feedback`. Every slice-3 feature (FT-027–FT-033) imports from here, not from siblings.

### Invariants

- Every persisted `Feedback` artifact passes the `dec:FeedbackShape` SHACL.
- Every `Feedback` carries `dec:feedbackClass`, `dec:lifecycleState`, `dec:targetRole`, `dec:evidence`, `dec:sourceSession`, `dec:inStream`.
- Every `Feedback` artifact's `dec:sourceSession` resolves to an existing `Session` artifact in the same stream.

### Error handling

- SHACL violation on write → `WriterError::ShaclViolation { report }`.
- Read against missing IRI → `FeedbackReadError::NotFound { iri }`.
- Malformed RDF in store (defensive) → `FeedbackReadError::Malformed { iri, detail }`.

### Boundaries

- **In scope.** Class definition, predicates, SHACL shape, Rust types, write validation, read helpers.
- **Out of scope.** Lifecycle state-machine validation ([FT-027](FT-027)). Class vocabulary `sh:in` ([FT-028](FT-028)). Routing ([FT-029](FT-029)). Workers/SDK ([FT-031](FT-031)). Dispatch-lifecycle interaction ([FT-032](FT-032)). CLI ([FT-033](FT-033)).

## Out of scope

- Feedback authoring through `product-cli` (Phase B at earliest — feedback is graph-resident; product-cli does not own it).
- Feedback storage outside the orchestration store (no external sinks).
- Feedback versioning / amendments (Phase B; for now feedback is monotonic per ADR-024).
