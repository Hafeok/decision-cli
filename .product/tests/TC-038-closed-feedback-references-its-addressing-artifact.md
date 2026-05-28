---
id: TC-038
title: closed feedback references its addressing artifact via PROV-O
type: invariant
status: passing
validates:
  features:
  - FT-027
  adrs:
  - ADR-024
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test feedback_closed_provo
runner-timeout: 120
last-run: 2026-05-25T23:43:40.429452005+00:00
last-run-duration: 0.2s
---

## Description

Invariant: every `dec:Feedback` artifact with `dec:state = "closed"` has a `dec:addressedBy` triple pointing at the artifact (a `CodeChange`, an amended `Feature`, a new `ADR`, etc.) that resolved the feedback, and a `prov:wasInvalidatedBy` triple pointing at the closure activity ([ADR-004](ADR-004) PROV-O).

A closed feedback without an `addressedBy` is a regression of [ADR-024](ADR-024)'s closure contract — the audit chain breaks and "how was this resolved" becomes unrecoverable.

## Runner

```sparql
PREFIX dec: <https://decision-cli.dev/ns/>
PREFIX prov: <http://www.w3.org/ns/prov#>
ASK WHERE {
  ?f a dec:Feedback ; dec:state "closed" .
  FILTER NOT EXISTS { ?f dec:addressedBy ?a }
}
```

ASK true → regression; runner exits 1.

⟦Γ:Invariants⟧{
  ∀ f:Feedback: f.state = closed ⇒
    ∃ a: addressedBy(f, a) ∧
    ∃ act: wasInvalidatedBy(f, act)
}