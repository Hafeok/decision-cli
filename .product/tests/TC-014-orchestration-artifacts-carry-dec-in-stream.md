---
id: TC-014
title: orchestration_artifacts_carry_dec_in_stream
type: invariant
status: failing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test tc_014_in_stream_invariant
runner-timeout: 60
---

## Purpose

Global invariant for decision-cli's graph per **ADR-005** (value stream as graph-resident scope, enforced at command time): every artifact in the orchestration graph of class `dec:Session`, `dec:Goal`, `dec:Dispatch`, or `dec:Event` must carry a `dec:inStream` link to the active `dec:ValueStream`.

Source: `decision-cli-slice-1-bounds.md` §11.2 invariant #14.

## Statement of invariant

For every `?a` such that `?a a ?cls` and `?cls IN (dec:Session, dec:Goal, dec:Dispatch, dec:Event)`:

```sparql
ASK { ?a dec:inStream ?stream . ?stream a dec:ValueStream }
```

MUST return `true`.

## How to verify

```sparql
SELECT ?a ?cls WHERE {
  VALUES ?cls { dec:Session dec:Goal dec:Dispatch dec:Event }
  ?a a ?cls .
  FILTER NOT EXISTS { ?a dec:inStream ?stream . ?stream a dec:ValueStream }
}
```

Result MUST be empty in every initialized store.

## When this invariant is checked

- After every test that mutates the orchestration store (TC-001, TC-002, TC-008, TC-009, TC-010).
- As a slice 1 exit gate / health audit in CI.

## Notes

- This invariant is the structural realisation of the §3.4 enforcement claim: the orchestrator cannot drift outside its declared scope.
- The complementary refusal-side claim is TC-007.
- FT-010's writer middleware is the implementation locus; this TC is the test that proves the middleware is wired everywhere it must be.

## Formal specification

⟦Σ:Types⟧{
  ScopedClass ≜ dec:Session | dec:Goal | dec:Dispatch | dec:Event
  Artifact ≜ ⟨id:IRI, type:IRI, stream:IRI?⟩
  ValueStream ≜ IRI
}

⟦Γ:Invariants⟧{
  ∀a:Artifact ∈ orchestration_store:
    a.type ∈ ScopedClass ⇒
      defined(a.stream) ∧ is_value_stream(a.stream)
  ∀a:Artifact ∈ orchestration_store:
    a.type ∈ ScopedClass ⇒
      a.stream = active_stream(orchestration_store)
}

⟦Ε⟧⟨δ≜0.95;φ≜95;τ≜◊⁺⟩
