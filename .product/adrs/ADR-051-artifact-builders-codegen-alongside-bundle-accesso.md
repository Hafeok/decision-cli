---
id: ADR-051
title: Artifact builders codegen alongside bundle accessors (one shape, two surfaces)
status: accepted
features:
- FT-080
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:5eacc28e9b0f9a01d482ce05ab053b42c0690cc7efda3bbaea7872350a39ba41
---

## Context

Typed artifact builders (FT-080) can be generated from SHACL output shapes,
the same way bundle accessors (FT-079) are generated from bundle shapes
(ADR-048). The shape declares what fields exist, which are required, and
what edges connect; both a reader and a writer can be derived from that
one source.

## Decision

Codegen artifact builders from output SHACL shapes using the same generator
that produces bundle accessors. One source of truth (the shape per role),
two generated surfaces (read-side accessors, write-side builders). Hand-
written escape hatch (`emit_triple`) for shape-conformant cases the typed
surface doesn't yet cover.

## Consequences

- **Positive:** Symmetric ergonomics on both sides of a session — workers
  read with typed accessors, write with typed builders, never touch raw
  triples for the common path.
- **Positive:** Conformance with the output shape is enforced before triples
  cross the wire (`builder.commit()` runs pyshacl locally), with the
  harness's GraphWriter as the authoritative re-validation point. Fast
  feedback loop without losing the boundary check.
- **Positive:** Adding a field to a role's output shape automatically
  surfaces a new setter on the builder, with required-field validation, in
  the next SDK release.
- **Negative:** Builders are only as good as the shapes. If a shape under-
  specifies what a role can emit, workers fall back to `emit_triple` and
  the surface gap is invisible until telemetry reports it.

## Alternatives considered

- **Hand-written builders per role.** Rejected: same drift risk as hand-
  written accessors. Defeats the purpose of having a single shape as the
  contract.
- **No builders; workers emit raw triples.** Rejected: every worker
  re-invents the boundary discipline, no defensive validation, errors
  surface at the harness instead of in worker code.

## References

- `feature:artifact-layer` (FT-080) is the consumer of this codegen output.
- ADR-048 (build-time codegen pipeline) — the same generator does both
  surfaces.