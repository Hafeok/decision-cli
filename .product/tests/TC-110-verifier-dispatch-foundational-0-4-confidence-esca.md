---
id: TC-110
title: Verifier dispatch foundational+0.4 confidence escalates twice into deep-reasoning
type: scenario
status: passing
validates:
  features:
  - FT-062
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test escalate_foundational_chain
runner-timeout: 240
last-run: 2026-05-23T16:10:16.715715484+00:00
last-run-duration: 0.2s
---

## Description

Scenario (PRD §11.2 bullet 3 + cache-hit assertion from §11.2 final bullet): a `verifier` dispatch with `stakes = foundational` and a worker returning `confidence = 0.4` escalates *twice* — first to `standard-reasoning-frontier` (via `stakes_elevated` / `confidence_below_0.7`, whichever fires first in the seed binding's escalation order), then to `deep-reasoning` (via `stakes_foundational` / `confidence_below_0.5`). All three sessions are linked in a chain. The Anthropic third tier records cache-write token counts (first time the prefix is seen).

The runner is `cargo-test` with the verifier worker stubbed to return `confidence = 0.4` on call 1, `confidence = 0.45` on call 2, and `confidence = 0.95` on call 3. The Anthropic SDK is stubbed at the API boundary to return a synthetic `usage` block including `cache_creation_input_tokens` and `cache_read_input_tokens`.

Acceptance:

1. **Setup.** Seed catalog (12 capabilities, 5 bindings). Compose a bundle whose focal artifact is a `dec:Capability` (so `default_stakes_for` returns `Foundational`). Confirm via `bundle.stakes == Foundational`. The bundle's static prefix (focal capability + linked ADRs + tool list) must exceed Anthropic's minimum cacheable size (~1024 tokens — pad with linked artifacts if needed).
2. **Stub worker** with the three canned responses above. Stub the Anthropic SDK for the S3 step to return `usage = {input_tokens: 200, output_tokens: 100, cache_creation_input_tokens: 3000, cache_read_input_tokens: 0}` (first cache write).
3. **Dispatch.** `dispatch_role(graph, "verifier", bundle)`.
4. **Three sessions.** Query for sessions in the dispatch group; assert exactly 3 rows.
5. **Capability progression.** S1.capability = `code-writer`, S2.capability = `standard-reasoning-frontier`, S3.capability = `deep-reasoning`. (Verifier's seed binding per PRD §6.2.)
6. **Chain linkage.** Assert `S1 → S2 → S3` via `escalated_to` and reverse via `escalated_from`. `escalation_chain(S3)` returns `[S1, S2, S3]`.
7. **Escalation reasons.** S2's reason is one of {`confidence_below_0.7`, `stakes_elevated`}. S3's reason is one of {`confidence_below_0.5`, `stakes_foundational`}.
8. **Bundle enrichment.** S2's bundle hash differs from S1's. S3's bundle hash differs from S2's. S3's enriched bundle contains a `Prior attempt (tier ?, capability standard-reasoning-frontier, …)` section.
9. **Endpoint progression.** S1 + S2 are `endpoint = scaleway`. S3 is `endpoint = anthropic` (per [ADR-037](ADR-037)).
10. **Cache breakpoint placement on S3 (PRD §11.2 / [FT-065](FT-065)).** The Anthropic SDK stub records the request payload; assert exactly one `cache_control: {"type": "ephemeral"}` marker is set, and it appears on the boundary between the stable prefix (system + focal artifact + linked ADRs + tool list) and the per-attempt suffix (the `Prior attempt` enrichment block).
11. **S3 session records cache fields.** `S3.input_tokens_base = 200`, `S3.input_tokens_cache_write = 3000`, `S3.input_tokens_cache_hit = 0` (first time the prefix is cached). `S3.output_tokens = 100`.
12. **S1 and S2 cache fields are zero.** `S1.input_tokens_cache_write = S1.input_tokens_cache_hit = 0` (Scaleway has no cache). Same for S2. SHACL enforces this per the scaleway-no-cache constraint in [FT-057](FT-057).
13. **cache_hit_rate sanity.** `cache_hit_rate(S3)` returns `0 / (200 + 3000 + 0) = 0.0` (first cache write, no hits yet). Subsequent same-prefix dispatch within 5 minutes (tested separately in [TC-114](TC-114)) would show a hit.
14. **Aggregate cost carries both currencies.** `aggregate_chain_cost(chain)` reports `{EUR: …, USD: …}`. The Scaleway tiers contribute to EUR; the Anthropic tier contributes to USD. The helper does not perform conversion at query time unless an explicit rate is passed.

⟦Σ:Types⟧{
  Chain ≜ Ordered List SessionRecord
  TokenBreakdown ≜ ⟨base:Nat, cacheWrite:Nat, cacheHit:Nat, output:Nat⟩
}

⟦Γ:Invariants⟧{
  stakes = foundational ∧ confidence < 0.5 ⇒ |chain| = 3
  chain[2].endpoint = anthropic
  chain[2].capability = deep-reasoning
  chain[2].input_tokens_cache_write > 0   -- first Anthropic dispatch in chain writes cache
  chain[0].input_tokens_cache_write = 0 ∧ chain[1].input_tokens_cache_write = 0   -- Scaleway has no cache
  aggregate_chain_cost(chain).byCurrency.has_key(EUR) ∧ aggregate_chain_cost(chain).byCurrency.has_key(USD)
}