---
id: TC-027
title: every ActionSession has a paired InterpretationSession via DispatchGroup
type: invariant
status: passing
validates:
  features:
  - FT-021
  - FT-022
  adrs:
  - ADR-017
phase: 2
runner: bash
runner-args: scripts/checks/action-interpretation-pairing.sh
runner-timeout: 60
last-run: 2026-05-24T19:14:23.673322616+00:00
last-run-duration: 0.1s
---

## Description

Invariant: every `dec:ActionSession` in the orchestration store has exactly one paired `dec:InterpretationSession` linked through a `dec:DispatchGroup`. The pairing is the structural enforcement of [ADR-017](ADR-017)'s action-interpretation requirement; without it, the verifier loop is non-closed and a dispatched implementer session has no counterpart.

The pairing is structural — every action session is `dec:groupedBy` a `DispatchGroup`, which in turn `dec:hasInterpretation` an interpretation session. A null pairing (action session with no group, or group with no interpretation) is the regression this TC catches.

## Runner

The runner script enumerates every action session and asserts the SPARQL ASK:

```sparql
PREFIX dec: <https://decision-cli.dev/ns/>
ASK WHERE {
  ?a a dec:ActionSession .
  ?a dec:groupedBy ?g .
  ?g dec:hasInterpretation ?i .
  ?i a dec:InterpretationSession .
}
```

If any action session lacks the chain, the script exits 1.

⟦Γ:Invariants⟧{
  ∀ a:ActionSession ∃! i:InterpretationSession, g:DispatchGroup:
    groupedBy(a, g) ∧ hasInterpretation(g, i)
}