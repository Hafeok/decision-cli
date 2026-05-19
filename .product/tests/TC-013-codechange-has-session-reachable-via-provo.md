---
id: TC-013
title: codechange_has_session_reachable_via_provo
type: invariant
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-013-codechange-session-provo.sh
runner-timeout: 120
last-run: 2026-05-19T09:46:07.737086591+00:00
last-run-duration: 0.4s
---

## Purpose

Global invariant: every `CodeChange` registered in product-cli's graph must have a corresponding `Session` record in decision-cli's graph reachable via PROV-O per **ADR-004**.

Source: `decision-cli-slice-1-bounds.md` §11.2 invariant #13.

## Statement of invariant

For every `CodeChange` artifact `?c` in product-cli's graph, there exists a `Session` `?s` in decision-cli's graph such that PROV-O lineage resolves `?c → ?s` (either directly via `prov:wasGeneratedBy` or transitively through PROV intermediates).

Cross-graph linkage is the new claim: the lineage spans **both** stores (product-cli's engineering graph and decision-cli's orchestration store).

## How to verify

A federated SPARQL query (or equivalent two-step lookup) executed against both stores:

1. Enumerate all `?c a dec:CodeChange` in product-cli's graph.
2. For each `?c`, resolve `?c prov:wasGeneratedBy ?s` and locate `?s a dec:Session` in decision-cli's graph.
3. Every `?c` MUST resolve to exactly one `?s`.

Pseudo-query:

```sparql
SELECT ?c WHERE {
  GRAPH <product-cli> { ?c a dec:CodeChange . }
  FILTER NOT EXISTS {
    GRAPH <product-cli> { ?c prov:wasGeneratedBy ?s }
    GRAPH <dec> { ?s a dec:Session }
  }
}
```

Result MUST be empty.

## When this invariant is checked

- After every TC-008 end-to-end run.
- As a slice 1 exit gate against any production-like fixture.

## Notes

- product-cli must surface the PROV link on `CodeChange` for this to be machine-verifiable; FT-011 calls product-cli's MCP write tools with the link as part of the `CodeChange` payload.
- TC-012 holds the dec-side invariant for Session.

## Formal specification

⟦Σ:Types⟧{
  CodeChange ≜ IRI
  Session ≜ IRI
  ProductGraph ≜ Set⟨Triple⟩
  DecGraph ≜ Set⟨Triple⟩
}

⟦Γ:Invariants⟧{
  ∀c:CodeChange ∈ ProductGraph:
    ∃!s:Session ∈ DecGraph:
      (c, prov:wasGeneratedBy, s) ∈ ProductGraph
      ∧ (s, rdf:type, dec:Session) ∈ DecGraph
}

⟦Ε⟧⟨δ≜0.85;φ≜80;τ≜◊⁺⟩