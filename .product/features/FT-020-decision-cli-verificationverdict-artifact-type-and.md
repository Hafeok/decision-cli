---
id: FT-020
title: 'decision-cli: VerificationVerdict artifact type and SHACL shape'
phase: 2
status: planned
depends-on:
- FT-001
- FT-006
adrs:
- ADR-013
- ADR-017
- ADR-018
tests:
- TC-029
- TC-030
domains: []
domains-acknowledged: {}
---

## Description

Land the `dec:VerificationVerdict` artifact type — schema, SHACL shape, write-side validation, read API. The verdict is the artifact that gates dispatch completion ([ADR-017](ADR-017)) and carries the three-value vocabulary defined in [ADR-018](ADR-018).

This is a pure schema-shaped feature: it extends the ontology and the `StreamWriter` validation paths; it does not by itself run any role or trigger any subscription. [FT-022](FT-022) and [FT-023](FT-023) make use of the schema.

## Functional Specification

### Inputs

- The embedded ontology ([FT-006](FT-006)) — extended in this feature.
- The `StreamWriter` chokepoint ([ADR-005](ADR-005), slice 1) — extended to recognize the new shape.
- The vocab module (`core::vocab`) — gains the new IRIs.

### Outputs

- New SHACL shape `dec:VerificationVerdictShape` per [ADR-018](ADR-018) §SHACL shape, embedded in the ontology bundle.
- New IRIs in `core::vocab`:
  - `dec:VerificationVerdict` (class)
  - `dec:verdict`, `dec:rationale`, `dec:violates`, `dec:amendmentGuidance` (properties)
  - `verdict:approved`, `verdict:rejected`, `verdict:amendment-required` (literals — used by Rust constructors)
- New Rust types under `core::ontology::verdict`:
  - `enum Verdict { Approved, Rejected, AmendmentRequired }`
  - `struct VerdictArtifact { verdict: Verdict, rationale: String, violates: Vec<ArtifactRef>, amendment_guidance: Option<String>, generated_by: SessionIri, used: Vec<ArtifactIri>, in_stream: StreamIri }`
  - `fn to_quads(&self, graph: NamedNodeRef) -> Vec<Quad>` — serialises to RDF for `StreamWriter`.
- `StreamWriter` validation path: when committing a `VerdictArtifact`, run SHACL inline before commit. Failures produce a structured error and abort.
- Read API: `core::read::list_verdicts_for_dispatch(store, dispatch_iri) -> Vec<VerdictArtifact>` and `core::read::latest_verdict_for_dispatch(...)`.

### State

- Persistent state changes: each new `VerificationVerdict` is a fresh artifact graph in the store, attached via PROV-O to its interpretation session ([FT-021](FT-021)) and to the artifacts it consumed.
- No backfill: existing slice-1 sessions have no verdicts and are out of scope.

### Behaviour

1. Add classes/properties/SHACL shape to the ontology bundle (extend the Turtle in `core/ontology/`).
2. Add IRI constants to `core::vocab`.
3. Add the `Verdict` enum, `VerdictArtifact` struct, and `to_quads` serialiser in `core::ontology::verdict`.
4. Extend `StreamWriter`:
   - Recognise `VerdictArtifact` mutations.
   - Run SHACL validation pre-commit (uses the existing ontology validation path from [FT-006](FT-006)).
   - Reject malformed verdicts with `WriterError::ShaclViolation` (existing variant).
5. Add the read helpers in `core::read`.
6. Per the slice-level SDP convention in `CLAUDE.md`, every consumer (FT-021, FT-022, FT-023, FT-024) imports from `core::ontology::verdict`; no consumer imports from sibling features.

### Invariants

- Every persisted `VerificationVerdict` passes the SHACL shape.
- Every `VerificationVerdict` carries exactly one `dec:verdict`, one `dec:rationale`, one `prov:wasGeneratedBy`, one `dec:inStream`, and ≥ 1 `prov:used`.
- `dec:verdict ∈ {approved, rejected, amendment-required}` enforced at SHACL.
- `rejected` and `amendment-required` verdicts carry ≥ 1 `dec:violates` reference.
- `amendment-required` verdicts carry exactly one `dec:amendmentGuidance` string.

### Error handling

- SHACL violation on write → `WriterError::ShaclViolation { report }` with the SHACL report rendered as text (matches [FT-006](FT-006)'s shape).
- Read API against a missing dispatch → returns empty `Vec` (not an error).
- Malformed RDF in the store (impossible under normal operation, but defensive) → `ReadError::MalformedVerdict { iri, detail }`.

### Boundaries

- **In scope.** Ontology extension, IRIs, Rust types, write-side validation, read helpers.
- **Out of scope.** Triggering verifier dispatches ([FT-022](FT-022)). Producing verdicts from a worker ([FT-023](FT-023)). Computing the agreement metric ([FT-024](FT-024)). CLI surface ([FT-025](FT-025)).

## Out of scope

- Confidence scoring on verdicts (rejected per ADR-018).
- Verdict supersession (an amended verdict replacing a prior one) — Phase B at earliest.
- Multi-verifier verdicts on the same dispatch (Phase C ensemble work).
