---
id: TC-028
title: DispatchGroup reaches complete only when both paired sessions are terminal
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
runner-args: scripts/checks/dispatch-complete-paired-terminal.sh
runner-timeout: 60
last-run: 2026-05-21T13:31:41.447481757+00:00
last-run-duration: 0.0s
---

## Description

Invariant: a `dec:DispatchGroup` only reaches `dec:state = "complete"` when **both** of its paired sessions (the action session and its interpretation session) are in terminal states. This is the lifecycle correctness claim of [ADR-017](ADR-017).

Terminal states for a session: `completed`, `failed`, `cancelled`. Non-terminal: `pending`, `in_progress`, `pending_review`. A `DispatchGroup` that flips to `complete` while either paired session is still non-terminal is a regression of the group's state machine.

## Runner

The runner script asserts the SPARQL ASK:

```sparql
PREFIX dec: <https://decision-cli.dev/ns/>
ASK WHERE {
  ?g a dec:DispatchGroup ; dec:state "complete" .
  { ?g dec:hasAction ?a .       ?a dec:state ?s . FILTER(?s NOT IN ("completed","failed","cancelled")) }
  UNION
  { ?g dec:hasInterpretation ?i . ?i dec:state ?s . FILTER(?s NOT IN ("completed","failed","cancelled")) }
}
```

The ASK returning `true` indicates a regression; the runner exits 1.

⟦Σ:Types⟧{
  SessionState ≜ pending | in_progress | pending_review | completed | failed | cancelled
  TerminalState ≜ completed | failed | cancelled
}

⟦Γ:Invariants⟧{
  ∀ g:DispatchGroup: state(g) = complete ⇒
    state(action(g)) ∈ TerminalState ∧ state(interpretation(g)) ∈ TerminalState
}