---
id: TC-103
title: SessionRecord escalation edges round-trip and chain integrity
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-057
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test session_escalation_edges
runner-timeout: 120
---

## Description

Invariant: `dec:SessionRecord` escalation edges ([FT-057](FT-057)) round-trip correctly, satisfy bidirectional consistency, and `escalation_chain` SPARQL helper walks the chain in dispatch order.

The runner is `cargo-test` and exercises:

1. **Bidirectional consistency at write.** Write S1, then S2 with `dec:escalated_from = S1` and `dec:escalation_reason = "confidence_below_0.7"`; the same transaction must add `dec:escalated_to = S2` to S1. Assert both edges are present after the write. Assert SHACL fails when only one direction is written (asymmetric).
2. **escalation_reason ↔ escalated_from coupling.** Write S2 with `escalated_from` set but `escalation_reason` absent. Assert SHACL `sh:sparql` violation. Write a root session (no `escalated_from`) with `escalation_reason` set. Assert SHACL violation.
3. **escalation_chain order.** Build a chain S1 → S2 → S3. Call `core::graph::session::escalation_chain(S2)`; assert it returns `[S1, S2, S3]` in order regardless of which session id is passed in (the helper walks both directions to find the root then forward to the leaf).
4. **aggregate_chain_cost.** Each session in the chain records `input_tokens`, `output_tokens`, and capability cost; assert `aggregate_chain_cost(chain)` sums them per the formula: `total_eur = Σ(input_tokens / 1e6 * cost_input_per_m) + Σ(output_tokens / 1e6 * cost_output_per_m)`.
5. **Orphan chain.** Write S2 referencing a non-existent S1; call `escalation_chain(S2)` and assert `SessionError::ChainBroken { session_id: S2, missing_ref: S1 }` is returned (helper does not panic).

⟦Σ:Types⟧{
  Chain ≜ Ordered NonEmpty List SessionRecord
  ChainCost ≜ ⟨inTokens:Nat, outTokens:Nat, eur:Decimal⟩
}

⟦Γ:Invariants⟧{
  ∀ S₁,S₂:SessionRecord: S₁.escalated_to = S₂ ⇔ S₂.escalated_from = S₁
  ∀ S:SessionRecord: S.escalation_reason ≠ ⊥ ⇔ S.escalated_from ≠ ⊥
  ∀ c:Chain: escalation_chain(c[k]) = c for all k ∈ indices(c)
}
