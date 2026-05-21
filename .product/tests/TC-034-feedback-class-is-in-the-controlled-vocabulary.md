---
id: TC-034
title: Feedback class is in the controlled vocabulary
type: invariant
status: passing
validates:
  features:
  - FT-028
  adrs:
  - ADR-023
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test feedback_class_vocab
runner-timeout: 120
last-run: 2026-05-21T13:31:41.447481757+00:00
last-run-duration: 5.4s
---

## Description

Invariant: every persisted `dec:Feedback` artifact's `dec:class` literal is in the controlled vocabulary defined by [ADR-023](ADR-023): `{gap, contradiction, ambiguity, defect, suggestion}`. The writer chokepoint is responsible for enforcing this at commit time; the invariant scans the persisted store for any drift.

## Runner

```sparql
PREFIX dec: <https://decision-cli.dev/ns/>
ASK WHERE {
  ?f a dec:Feedback ; dec:class ?c .
  FILTER(?c NOT IN ("gap","contradiction","ambiguity","defect","suggestion"))
}
```

ASK returns `true` → regression in writer enforcement; runner exits 1.

⟦Σ:Types⟧{
  FeedbackClass ≜ gap | contradiction | ambiguity | defect | suggestion
}

⟦Γ:Invariants⟧{
  ∀ f:Feedback: f.class ∈ FeedbackClass
}