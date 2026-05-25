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
tests: []
domains: []
domains-acknowledged: {}
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