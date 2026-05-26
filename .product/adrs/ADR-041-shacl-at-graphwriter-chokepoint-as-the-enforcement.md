---
id: ADR-041
title: SHACL at GraphWriter chokepoint as the enforcement mechanism for provenance discipline
status: accepted
features:
- FT-072
- FT-073
- FT-001
supersedes: []
superseded-by: []
domains:
- data-model
- error-handling
scope: cross-cutting
content-hash: sha256:886e1992c1692fa2c513edd7754b5921336b1edecc1e2f2ce064b917d5dad474
---

## Context

ADR-038 establishes the dual-provenance discipline. The discipline is only meaningful if it is *enforced* — an aspirational rule the graph "should follow" decays to noise within weeks. Three enforcement options:

| Option | Mechanism | Invariant strength | Failure mode |
|---|---|---|---|
| **SHACL at write time** | Shape validation runs as part of every GraphWriter commit. Non-conformant writes are rejected. | Graph never contains non-conformant artifacts. | Producer must repair before writing; immediate feedback. |
| **Runtime checks in code** | Each artifact-producing code path validates before calling GraphWriter. | Per-call-site; depends on every author remembering. | Easy to forget; distributed enforcement; silent drift. |
| **Periodic audit** | Scan the graph periodically; emit feedback for non-conformance. | Eventually consistent. | Doesn't prevent bad writes — just surfaces them after the fact. |

## Decision

**SHACL validation runs at the GraphWriter chokepoint on every mutation. Non-conformant writes are rejected before commit.**

GraphWriter (decision-cli's FT-001, the single mutation chokepoint mandated by `docs/ddd/Implementing_DDD.md` §7) gains a SHACL validation stage between transaction prepare and commit. Validation has three failure modes:

1. **Missing mechanical provenance.** GraphWriter is the *source* of the mechanical triples — it materializes them from the session record handed in by the harness. A missing mechanical block is therefore an internal bug, not a producer error. Treat as a `panic!`-equivalent assertion failure: hard error, log, exit; never reachable in a correctly-wired system.

2. **Missing motivational provenance AND not a BoundaryArtifact.** Write is rejected. GraphWriter returns a structured `ProvenanceViolation` containing:
   - The artifact ID and declared `rdf:type`.
   - The set of motivational predicates the type's shape accepts (from FT-070's vocabulary).
   - The fact that none were present and the artifact was not declared `:BoundaryArtifact`.
   The violation is itself emitted as a Feedback artifact (ADR-022) routed back to the producing session — or to the external submitter for boundary-rejection cases (CI worker image submissions, etc.).

3. **Type-specific shape violations** — required field missing, edge pointing at a wrong-typed target, `:external_origin` missing on a `BoundaryArtifact`, etc. Same rejection-and-feedback pattern; the violation report names the failed `sh:property` paths.

### Validator choice and dual-side enforcement

- **Authoritative validator: oxigraph-shacl (Rust).** Runs inside GraphWriter on the harness side. Same shape files as the SDK validator; same SHACL spec. This is the validator whose verdict commits or rejects.
- **Defensive validator: pyshacl (Python).** Runs inside the SDK in workers before they hand artifacts off via the worker contract (ADR-008). Same shape files. Catches violations one tier earlier, surfaces them as worker-side errors before crossing the worker/harness boundary, but is not authoritative — the harness re-validates and is the source of truth.

Both sides loading the same `shapes/*.ttl` files is enforced by a build-time fitness function: the shape file set is single-sourced under `crates/decision-cli/src/core/ontology/shapes/`, both Rust and Python builds reference that directory, and a CI check fails if either side has a divergent copy.

### Cross-artifact constraints evaluated against live snapshot

Validation runs on the incoming triple set *composed with* the current named-graph snapshot, so cross-artifact constraints (the target of an `addresses → Feedback` edge must exist and be of class `Feedback`) are checked against the live graph. GraphWriter holds the snapshot for the duration of the transaction; reads are serializable with the write under evaluation.

### Choice of SHACL specifically

Alternatives considered: SHEX, custom validators, JSON Schema, ad-hoc Rust matchers.

- **SHEX.** Comparable expressiveness; weaker tool ecosystem.
- **Custom validators.** Distributes shape knowledge across code; loses declarative composition.
- **JSON Schema.** Wrong substrate — operates on JSON, not RDF.
- **Ad-hoc Rust matchers.** Imperative; cannot be re-used by the Python SDK side without re-implementation.

SHACL is the W3C standard for RDF validation, has mature implementations in both Python (pyshacl) and Rust (oxigraph-shacl), composes declaratively via `sh:and` / `sh:or` / `sh:node`, supports the property-path expressions the discipline needs. No serious alternative for this substrate.

### Performance budget

Slice-1 validation latency target: < 50 ms p99 added to a GraphWriter commit. The mechanical block is a fixed shape with three properties; per-type shapes have at most ~10 alternatives in the `sh:or` block. Cross-artifact constraints add a few SPARQL ASK queries against the live graph. This budget is met by oxigraph-shacl on the slice-1 reference workload; revisit if shape complexity grows.

## Consequences

**Positive.**

- The graph maintains the invariant "every artifact conforms to its shape" *continuously*, not eventually.
- Producers learn about violations immediately, at the boundary that caused them. No silent drift; no surprise audit findings months later.
- Dual-validator setup catches violations on both sides of the SDK/harness boundary, keeping the worker-contract surface (ADR-008) clean.
- New artifact types ship with a shape file and immediately participate in the discipline — no separate "register this type with the validator" step.

**Negative / accepted costs.**

- Every GraphWriter commit pays the validation cost (target < 50ms p99). For batch loads this is non-trivial; mitigation is to batch the validation as well (validate the full triple-delta in one pass per commit, not per-artifact).
- Shape files become a load-bearing single source of truth that two implementations must agree on. Build-time fitness function prevents divergence; an actual incident would still hurt.
- Producers blocked by validation must repair *before* writing — debugging cycle gains a hop. Mitigation: pyshacl defensive validation on the SDK side surfaces most violations one tier earlier with better diagnostics.

**Boundary enforcement.** The chokepoint (GraphWriter) is the discipline. Any mutation path that bypasses GraphWriter bypasses the discipline. Slice-2's continuous-orphan fitness function (excluded from slice 1 per the Brief) scans for triples lacking the mechanical block as evidence of chokepoint bypass.

## Relationship to existing ADRs

- **ADR-038 (dual provenance).** This ADR is how ADR-038's invariants are enforced.
- **ADR-008 (worker contract: stateless bundle-in, artifact-out).** Compatible — workers hand artifacts back; the harness writes them via GraphWriter; validation runs at write. Workers do not need graph access to validate.
- **ADR-014 (Architectural fitness functions tracked as artifacts).** Compatible — the slice-2 continuous-orphan check is a fitness function in the ADR-014 sense.

## Status

Proposed. Implementation in FT-072 (shape files) and FT-073 (GraphWriter validation stage). The build-time dual-validator-agreement fitness function is a slice-1 deliverable per the Brief.
