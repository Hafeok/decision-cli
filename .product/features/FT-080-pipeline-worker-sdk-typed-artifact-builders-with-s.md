---
id: FT-080
title: 'pipeline-worker SDK: Typed artifact builders with SHACL validation at commit'
phase: 3
status: planned
depends-on:
- FT-078
- FT-085
- FT-072
- FT-073
adrs:
- ADR-051
- ADR-048
- ADR-013
- ADR-016
tests:
- TC-145
domains: []
domains-acknowledged:
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. Typed builders mapped to each
role's output SHACL shape, so workers never assemble RDF triples by hand for
the structured cases. Addresses ADR-051 (artifact builder codegen) and
ADR-048 (build-time codegen pipeline).

Depends on the dual-provenance discipline:
- FT-072 ships the shape files (per-type SHACL including motivational-predicate
  constraints) that the codegen reads.
- FT-073 ensures the harness's GraphWriter re-validates incoming artifacts
  against those same shapes at the boundary — this Feature's local pyshacl
  pass is the defensive check; FT-073's check is authoritative.

## Location

`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/artifact/` — the
generated builder modules are checked in here, one per role output shape.

## Scope

- Generated `ArtifactBuilder` per role output shape:
  - `artifact.set_title(...)`, `artifact.set_<field>(...)` for required fields
  - `artifact.set_motivated_by(...)` / `artifact.set_decomposes_from(...)` /
    etc. for the role-relevant motivational predicates from FT-070's
    vocabulary (the builder enforces "at least one motivational predicate"
    per ADR-038's discipline, except for `BoundaryArtifact` outputs governed
    by ADR-040).
  - `artifact.link_to(uri, predicate=...)` for additional edges declared in
    the shape.
  - `artifact.commit()` — runs pyshacl conformance against the role's output
    shape (including the motivational fragment) and surfaces failures as
    exceptions before triples ever cross the wire.
- Defensive validation on the worker side — the harness's GraphWriter
  re-validates on receive (FT-073, authoritative check at the boundary).
- Mechanical provenance (`prov:wasGeneratedBy`, `prov:wasAttributedTo`,
  `prov:generatedAtTime`) is NOT emitted by the builder — the harness
  populates it from the session record per ADR-050 / FT-069.
- `artifact.emit_triple(s, p, o)` escape hatch for shape-conformant cases the
  typed surface doesn't yet cover; flagged in telemetry like
  `bundle.raw_store`.

## Out of scope

- Hand-written role-specific builders (codegen output of FT-085).
- Cross-artifact transactions (one builder ⇒ one artifact per `commit()`).
- Emitting mechanical provenance from the worker (harness side, FT-069/073).

## Success criteria

- A builder missing a required field — including a required motivational
  predicate — raises on `commit()` with a SHACL-derived error message,
  before any wire send.
- A builder that passes local pyshacl conformance also passes the harness's
  re-validation on receive (same shape, same engine — no semantic drift).
- Calls to `emit_triple` increment a telemetry counter visible on the
  completion event.