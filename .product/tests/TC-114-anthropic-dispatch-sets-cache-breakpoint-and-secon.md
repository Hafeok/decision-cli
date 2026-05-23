---
id: TC-114
title: Anthropic dispatch sets cache breakpoint and second escalated session records cache_hit_input_tokens > 0
type: exit-criteria
status: passing
validates:
  features:
  - FT-065
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test anthropic_cache_breakpoint
runner-timeout: 180
last-run: 2026-05-23T16:10:19.599685124+00:00
last-run-duration: 0.2s
---

## Description

Scenario (PRD §11.2 final bullet): an Anthropic dispatch with a stable bundle prefix sets a cache breakpoint between the bundle's static portion and the per-attempt suffix; the second escalated session within 5 minutes records `input_tokens_cache_hit > 0` on its session record per [FT-065](FT-065) / [FT-057](FT-057).

The runner is `cargo-test` with the Anthropic SDK stubbed at the API boundary. The stub:

- Inspects the request payload for the `cache_control: {"type": "ephemeral"}` marker and records *where* it appeared.
- Simulates Anthropic's cache state across two requests within a 5-minute window: first request returns `usage.cache_creation_input_tokens > 0` and `cache_read_input_tokens = 0`; second request returns `cache_creation_input_tokens = 0` and `cache_read_input_tokens > 0` (the cached prefix is read back).

Acceptance:

1. **Setup.** Seed the catalog ([FT-058](FT-058)). Compose a bundle with `stakes = "foundational"` and a sizeable static prefix (focal `dec:Capability` artifact + 3 linked ADRs + the tool list — sufficient to exceed Anthropic's minimum-cache-block threshold of ~1024 tokens).
2. **Stub setup.** Configure the Anthropic worker stub to:
   - Verify the request includes exactly one `cache_control: {"type": "ephemeral"}` marker on the system / first-message block boundary.
   - On call 1, return `usage = {input_tokens: 100, output_tokens: 50, cache_creation_input_tokens: 2000, cache_read_input_tokens: 0}` (cache write).
   - On call 2 (escalated), return `usage = {input_tokens: 200, output_tokens: 50, cache_creation_input_tokens: 0, cache_read_input_tokens: 2000}` (cache read).
   - Stub the worker's verdict to drive escalation: confidence 0.4 on call 1 (triggers escalation to deep-reasoning per the verifier binding), confidence 0.95 on call 2 (terminates).
3. **Dispatch.** `dispatch_role(graph, "verifier", bundle)` with the foundational stakes. The verifier's seed binding will reach `deep-reasoning` (Anthropic) as the second tier (skipping `standard-reasoning-frontier` for brevity in this test — or alternatively, this TC accepts the chain `code-writer → standard-reasoning-frontier → deep-reasoning` and asserts cache behavior on the Anthropic step only).
4. **Cache breakpoint placement asserted.** The stub records that the `cache_control` marker appears on the request only when the resolved capability has `endpoint = anthropic` AND `cost_cache_hit_per_m` is non-null. For the Scaleway tier(s) of the chain, no marker is sent.
5. **Session record cache breakdown.** For the Anthropic-tier session(s), the `dec:SessionRecord` carries:
   - `dec:input_tokens_base > 0` (the 100 / 200 from the stub).
   - `dec:input_tokens_cache_write > 0` on the first Anthropic dispatch in the chain.
   - `dec:input_tokens_cache_hit > 0` on the second Anthropic dispatch in the chain (if the chain reaches Anthropic twice; otherwise this assertion runs against a contrived two-Anthropic-dispatch sequence using the same stub).
6. **Scaleway sessions have zero cache fields.** Earlier tiers' sessions (Scaleway) have `input_tokens_cache_write = 0` and `input_tokens_cache_hit = 0` enforced by SHACL ([FT-057](FT-057)).
7. **cache_hit_rate metric.** `core::graph::session::cache_hit_rate(anthropic_session_id)` returns a value in `[0.0, 1.0]`; for the cache-read session in this test, the value is `2000 / (200 + 0 + 2000) ≈ 0.909`, above the ADR-037 threshold of 0.70.
8. **Aggregate cost reflects cache savings.** `aggregate_chain_cost(chain)` reports `cache_hit_input_tokens * cost_cache_hit_per_m` as a distinct line; the EUR/USD currency tracking from [FT-054](FT-054) means the rollup carries both currencies (Scaleway tier in EUR, Anthropic tier in USD).
9. **No cache on Scaleway.** Same test setup with a Scaleway-only chain (no foundational stakes, low confidence so it escalates within Scaleway). Assert no `cache_control` marker ever sent; all cache fields stay 0 on session records.

⟦Σ:Types⟧{
  CacheUsage ≜ ⟨base:Nat, cacheWrite:Nat, cacheHit:Nat, output:Nat⟩
  ChainCost ≜ ⟨perTier:List (Endpoint, CacheUsage, Currency), totalsByCurrency:Map Currency Decimal⟩
}

⟦Γ:Invariants⟧{
  capability.endpoint = anthropic ∧ capability.cost_cache_hit_per_m ≠ ⊥ ⇒ request_payload.has_cache_control_marker
  capability.endpoint = scaleway ⇒ session.input_tokens_cache_write = 0 ∧ session.input_tokens_cache_hit = 0
  ∀ session: session.input_tokens_cache_hit ≤ total_input_tokens(session)
  cache_hit_rate(session) ∈ [0.0, 1.0]
}