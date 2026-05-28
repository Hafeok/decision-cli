---
id: TC-035
title: Feedback lifecycle transitions are validated
type: invariant
status: passing
validates:
  features:
  - FT-027
  adrs:
  - ADR-024
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test feedback_lifecycle
runner-timeout: 180
last-run: 2026-05-25T23:43:40.429452005+00:00
last-run-duration: 0.3s
---

## Description

Invariant: every state transition of a `dec:Feedback` artifact respects the lifecycle state machine defined in [ADR-024](ADR-024). The valid transitions are:

- `open → acknowledged`
- `open → closed` (direct close)
- `acknowledged → closed`
- (terminal) `closed`

Illegal transitions (e.g. `closed → open`, `closed → acknowledged`) must be refused at the writer. The invariant queries the audit trail (PROV-O activities recording transitions) and asserts every recorded transition is in the legal set.

## Runner

A `cargo-test` integration loads the store, enumerates every transition event, and matches against the legal-transition table.

⟦Σ:Types⟧{
  FeedbackState ≜ open | acknowledged | closed
  Transition ≜ ⟨from:FeedbackState, to:FeedbackState⟩
  LegalTransition ≜ ⟨open,acknowledged⟩ | ⟨open,closed⟩ | ⟨acknowledged,closed⟩
}

⟦Γ:Invariants⟧{
  ∀ t:Transition ∈ audit_trail: t ∈ LegalTransition
}