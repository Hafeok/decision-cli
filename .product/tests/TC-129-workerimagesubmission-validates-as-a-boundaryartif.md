---
id: TC-129
title: WorkerImageSubmission validates as a BoundaryArtifact
type: exit-criteria
status: failing
validates:
  features:
  - FT-087
  adrs:
  - ADR-040
  - ADR-055
phase: 3
runner: cargo-test
runner-args: tc_129_workerimagesubmission_validates_as_a_boundaryartif
runner-timeout: 120
last-run: 2026-05-28T08:48:38.944742131+00:00
last-run-duration: 0.3s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Purpose

Exit criterion for [FT-087](FT-087) (WorkerImageSubmission as the
initial-request artifact for admission) and validation of [ADR-040](ADR-040)
(BoundaryArtifact escape hatch) and [ADR-055](ADR-055) (WorkerImage
mirrors the Model catalog).

## Given

- A fresh Oxigraph store seeded only with the runtime ontology.
- One or more `dec:WorkerImageSubmission` artifacts constructed in memory
  and serialised to RDF quads via [`WorkerImageSubmission::to_quads`].

## When

```bash
cargo test -p decision-cli --test tc_129_workerimagesubmission_validates_as_a_boundaryartif
```

## Then

1. A well-formed `dec:WorkerImageSubmission` admits via the FT-087
   field-level SHACL validator (`validate_quads`) AND the FT-071 /
   ADR-040 `:BoundaryArtifactShape` `dec:external_origin` validator.
2. The serialised form carries the BoundaryArtifact class membership
   the per-type shape's `sh:or` requires — via the co-declared
   `rdf:type dec:InitialRequest` and the `dec:InitialRequest
   rdfs:subClassOf dec:BoundaryArtifact` chain.
3. SHACL refuses Submissions missing required fields:
   - `dec:candidate_registry_ref` without `@sha256:` digest.
   - zero `dec:claimed_capability_tag` literals.
   - Unknown `dec:submission_lifecycle_state` value (not in the
     `{received, under-review, admitted, rejected}` enum).
   - Empty `dec:external_origin`.
4. A Submission in the `admitted` lifecycle state with a
   `dec:produced_workerimage` edge round-trips cleanly through the
   serialiser and admits SHACL.

## Notes

The test owns its own in-memory store and constructs artifacts via the
public API (`WorkerImageSubmission::to_quads`, `Store::insert`); no graph
mutations escape the test scope. Lifecycle enforcement (Curator-only
state transitions) lives in FT-092 (the Curator session), not here —
this TC only validates that the persisted state value is in the declared
enum.

## Formal specification

⟦Σ:Types⟧{
  Submission ≜ dec:WorkerImageSubmission
  State ≜ {received, under-review, admitted, rejected}
}

⟦Γ:Invariants⟧{
  ∀ s:Submission:
    well_formed(s) ⇒ validate_quads(s.to_quads(...)) = Ok
  ∀ s:Submission:
    s ∈ instances_of(dec:BoundaryArtifact)   ; via subclass chain
  ∀ s:Submission:
    s.lifecycle_state ∈ State
}