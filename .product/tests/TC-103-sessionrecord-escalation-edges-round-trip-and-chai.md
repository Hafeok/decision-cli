---
id: TC-103
title: SessionRecord escalation edges round-trip and chain integrity
type: exit-criteria
status: passing
validates:
  features:
  - FT-057
  adrs:
  - ADR-034
phase: 2
runner: cargo-test
runner-args: tc_103_session_escalation_edges_round_trip_and_chain_integrity
runner-timeout: 120
last-run: 2026-05-25T23:43:40.429452005+00:00
last-run-duration: 0.5s
---

## Description

Invariant: `dec:SessionRecord` escalation edges + cache-aware token breakdown ([FT-057](FT-057)) round-trip correctly, satisfy bidirectional consistency, and `escalation_chain` + `aggregate_chain_cost` + `cache_hit_rate` SPARQL helpers behave as specified.

The runner is `cargo-test` and exercises:

1. **Bidirectional consistency at write.** Write S1, then S2 with `dec:escalated_from = S1` and `dec:escalation_reason = "confidence_below_0.7"`; the same transaction must add `dec:escalated_to = S2` to S1. Assert both edges are present after the write. Assert SHACL fails when only one direction is written (asymmetric).
2. **escalation_reason ↔ escalated_from coupling.** Write S2 with `escalated_from` set but `escalation_reason` absent. Assert SHACL `sh:sparql` violation. Write a root session (no `escalated_from`) with `escalation_reason` set. Assert SHACL violation.
3. **escalation_chain order.** Build a chain S1 → S2 → S3. Call `core::graph::session::escalation_chain(S2)`; assert it returns `[S1, S2, S3]` in order regardless of which session id is passed in (the helper walks both directions to find the root then forward to the leaf).
4. **Token-breakdown fields present and validated.** Write a session with `input_tokens_base = 100`, `input_tokens_cache_write = 0`, `input_tokens_cache_hit = 0` (Scaleway). SHACL passes. Write the same session with `input_tokens_cache_hit = 50` while capability resolves to Scaleway. Assert SHACL `sh:sparql` violation per the scaleway-no-cache constraint from [FT-057](FT-057).
5. **Anthropic cache fields populated.** Write a session whose capability resolves to `deep-reasoning` (Anthropic), with `input_tokens_base = 100`, `input_tokens_cache_write = 2000`, `input_tokens_cache_hit = 0` (cache-write case). SHACL passes. Write a second session with `input_tokens_base = 200`, `input_tokens_cache_write = 0`, `input_tokens_cache_hit = 2000` (cache-read case). SHACL passes.
6. **cache_hit_rate computation.** For the cache-read session above, `cache_hit_rate(session_id)` returns `2000 / (200 + 0 + 2000) ≈ 0.909`. For the cache-write session, the rate is `0 / 2100 = 0.0`. For a Scaleway session with no cache fields, the rate is `0.0` (not NaN).
7. **aggregate_chain_cost.** Each session in the chain records all four token-count fields (base / cache_write / cache_hit / output) and the resolving capability has its cost-rate fields ([FT-054](FT-054)). Assert `aggregate_chain_cost(chain)` sums them per the formula: `total_native_currency = Σ over chain ((base * cost_input_per_m + cache_write * cost_cache_write_5m + cache_hit * cost_cache_hit_per_m + output * cost_output_per_m) / 1e6)`. Per-currency totals are computed separately (EUR for Scaleway tiers, USD for Anthropic tiers); the helper returns a `{ByCurrency: { EUR: …, USD: … }}` map, not a single converted total.
8. **Orphan chain.** Write S2 referencing a non-existent S1; call `escalation_chain(S2)` and assert `SessionError::ChainBroken { session_id: S2, missing_ref: S1 }` is returned (helper does not panic).

⟦Σ:Types⟧{
  TokenBreakdown ≜ ⟨base:Nat, cacheWrite:Nat, cacheHit:Nat, output:Nat⟩
  Chain ≜ Ordered NonEmpty List SessionRecord
  ChainCost ≜ ⟨byCurrency:Map Currency Decimal⟩
}

⟦Γ:Invariants⟧{
  ∀ S₁,S₂:SessionRecord: S₁.escalated_to = S₂ ⇔ S₂.escalated_from = S₁
  ∀ S:SessionRecord: S.escalation_reason ≠ ⊥ ⇔ S.escalated_from ≠ ⊥
  ∀ c:Chain: escalation_chain(c[k]) = c for all k ∈ indices(c)
  ∀ S where S.capability.endpoint = scaleway: S.input_tokens_cache_write = 0 ∧ S.input_tokens_cache_hit = 0
  ∀ S: cache_hit_rate(S) ∈ [0.0, 1.0]
}