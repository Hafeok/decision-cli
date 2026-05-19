---
id: TC-015
title: bootstrap_session_reachable_from_valuestream_via_provo
type: invariant
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test tc_015_bootstrap_session
runner-timeout: 60
last-run: 2026-05-19T12:13:07.298280911+00:00
last-run-duration: 1.0s
---

## Purpose

Bootstrap-session invariant: the bootstrap session record `dec:session/init-001` is present in **every initialized store** (FT-008 / FT-009), and is reachable via PROV-O (**ADR-004**) from the `dec:ValueStream` artifact.

Source: `decision-cli-slice-1-bounds.md` §11.2 invariant #15.

## Statement of invariant

For every initialized orchestration store, both of the following hold:

1. `dec:session/init-001` exists and is typed `dec:Session` (and `prov:Activity`).
2. The active `dec:ValueStream` artifact is reachable from `dec:session/init-001` via PROV-O lineage — i.e., the bootstrap session is the activity that **generated** the ValueStream artifact.

## How to verify

```sparql
ASK {
  <dec:session/init-001> a dec:Session .
  ?stream a dec:ValueStream ;
          prov:wasGeneratedBy <dec:session/init-001> .
}
```

MUST return `true` in any store produced by `dec init` (TC-001 or TC-002 path).

Negative check (must be empty):

```sparql
SELECT ?stream WHERE {
  ?stream a dec:ValueStream
  FILTER NOT EXISTS {
    <dec:session/init-001> a dec:Session .
    ?stream prov:wasGeneratedBy <dec:session/init-001>
  }
}
```

## When this invariant is checked

- Immediately after any `dec init` (TC-001, TC-002).
- As a startup-time integrity check on any pre-existing store (slice 2 candidate; informational in slice 1).

## Notes

- This invariant pairs the bootstrap session to the ValueStream it produced; the PROV-O chain is what makes `dec status` (TC-006) able to reproduce the source path, content hash, and ontology version.
- The complementary general-Session invariant is TC-012.

## Formal specification

⟦Σ:Types⟧{
  BootstrapId ≜ <dec:session/init-001>
  Store ≜ Set⟨Triple⟩
  ValueStream ≜ IRI
}

⟦Γ:Invariants⟧{
  ∀store:Store ∈ initialized_stores:
    (BootstrapId, rdf:type, dec:Session) ∈ store
    ∧ ∃vs:ValueStream:
        (vs, rdf:type, dec:ValueStream) ∈ store
        ∧ (vs, prov:wasGeneratedBy, BootstrapId) ∈ store
  ∀store:Store: |{vs | (vs, prov:wasGeneratedBy, BootstrapId) ∈ store ∧ (vs, rdf:type, dec:ValueStream) ∈ store}| = 1
}

⟦Ε⟧⟨δ≜0.95;φ≜90;τ≜◊⁺⟩