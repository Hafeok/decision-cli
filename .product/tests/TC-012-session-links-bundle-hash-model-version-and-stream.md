---
id: TC-012
title: session_links_bundle_hash_model_version_and_stream_via_provo
type: invariant
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test tc_012_session_invariants
runner-timeout: 120
last-run: 2026-05-19T09:46:04.507476245+00:00
last-run-duration: 1.4s
---

## Purpose

Global invariant for every `Session` record (FT-011) in decision-cli's graph: the Session must link to its bundle hash, model version, and value stream via PROV-O per **ADR-004** (and the `dec:inStream` triple per **ADR-005**).

Source: `decision-cli-slice-1-bounds.md` §11.2 invariant #12.

## Statement of invariant

For every triple pattern `?s a dec:Session`, **all** of the following queries return non-empty:

1. ```sparql
   ASK { ?s prov:used ?bundleRef . ?bundleRef dec:contentHash ?h }
   ```
2. ```sparql
   ASK { ?s prov:used ?modelRef . ?modelRef dec:modelVersion ?v }
   ```
3. ```sparql
   ASK { ?s dec:inStream ?stream . ?stream a dec:ValueStream }
   ```

The exact predicate names are illustrative; the invariant holds against whichever concrete vocabulary FT-011 settles on, provided each of the three required links is present.

## How to verify

A graph-level audit query, runnable after any test that produces Sessions:

```sparql
SELECT ?s WHERE {
  ?s a dec:Session .
  FILTER NOT EXISTS { ?s prov:used ?bundle . ?bundle dec:contentHash ?h }
  UNION
  FILTER NOT EXISTS { ?s prov:used ?model . ?model dec:modelVersion ?v }
  UNION
  FILTER NOT EXISTS { ?s dec:inStream ?stream }
}
```

Result MUST be empty.

## When this invariant is checked

- After every implementer run (e.g., as part of TC-008).
- As a standalone audit in CI against a populated test fixture.

## Notes

- The bootstrap session (`dec:session/init-001`) is the special case covered by TC-015.
- TC-013 covers the complementary CodeChange-side invariant.
- TC-014 covers the broader `dec:inStream` claim across Session/Goal/Dispatch/Event.

## Formal specification

⟦Σ:Types⟧{
  Hash ≜ String
  ModelVersion ≜ String
  StreamIRI ≜ IRI
  BundleRef ≜ ⟨ref:IRI, hash:Hash⟩
  ModelRef ≜ ⟨ref:IRI, version:ModelVersion⟩
  Session ≜ ⟨id:IRI, bundle:BundleRef, model:ModelRef, stream:StreamIRI⟩
}

⟦Γ:Invariants⟧{
  ∀s:Session:
      defined(s.bundle.hash)
    ∧ defined(s.model.version)
    ∧ defined(s.stream)
    ∧ is_value_stream(s.stream)
  ∀s:Session: s.bundle.hash = sha256(bundle_bytes(s.bundle.ref))
}

⟦Ε⟧⟨δ≜0.9;φ≜85;τ≜◊⁺⟩