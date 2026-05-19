---
id: TC-019
title: dec init seeds v0 bootstrap subscriptions
type: invariant
status: passing
validates:
  features:
  - FT-009
  adrs:
  - ADR-003
phase: 1
runner: cargo-test
runner-args: --test tc_019_bootstrap_subscriptions
runner-timeout: 60
last-run: 2026-05-19T12:13:07.298280911+00:00
last-run-duration: 0.3s
---

## Description

`dec init` is the single moment the orchestration store is bootstrapped. FT-009 §Behaviour step 4 makes the v0 bootstrap subscriptions ("dispatch available for code-writer" and "code-writer dispatch completed") part of that bootstrap — they are what makes events flow on the very first `dec implement` run.

The first headless implementation of FT-009 marked the feature `complete` without persisting any subscription artifacts. The existing TCs (TC-001, TC-002, TC-015) all passed because none of them assert anything about subscriptions. TC-019 closes that loophole: a future regression that ships an "init" path without seeding subscriptions must fail this test.

The check lives in the test suite (not as a runtime invariant in the binary) because seeding is a one-shot bootstrap concern — the orchestration store can legitimately drop a subscription at runtime via `GraphWriter::remove_subscription`, and an invariant at use-time would conflate "removed by design" with "never seeded".

## Given

- A throwaway tempdir with no prior `.dec/` and no `.product/`.
- The decision-cli workspace builds cleanly.
- `CODE_WRITER_STUB=1` is set in the test process so the worker is deterministic.

## When

```bash
cargo test --test tc_019_bootstrap_subscriptions
```

The test runs `decision_cli::init::run(&tempdir, Template("engineering-development"))` and then `decision_cli::implement::run(&tempdir, ImplementArgs::new("FT-013"))`.

## Then

After `dec init` returns success, the persisted `.dec/store/orchestration.nq` must satisfy:

1. **Exact count.** Exactly **two** `oxi:Subscription` instances exist in `<https://decision-cli.dev/oxi-events/ns/subscriptions>`, with IRIs:
   - `https://decision-cli.dev/ns/subscription/dispatch-available-code-writer`
   - `https://decision-cli.dev/ns/subscription/dispatch-completed-code-writer`
2. **Graph location.** Both live in the `oxi-events:subscriptions` named graph (ADR-003), not the default graph.
3. **Query kind.** Each carries a non-empty `oxi:selectQuery` literal — and **no** `oxi:askQuery`. FT-009 mandates SELECT semantics so the evaluator diffs against the cached prior result set.
4. **Mode.** Each carries `oxi:mode "inline"`.

After re-opening the persisted store:

5. `GraphWriter::open(store).registry().len()` returns `2`. The persisted form is also the registered form (`SubscriptionRegistry::load_from_store` round-trip).

After one stub `dec implement` run:

6. `replay(post_store, ReplayRequest::since(0))` returns **at least two** events, one per seeded subscription. Steps 1-5 alone would be satisfied by dormant subscriptions; step 6 is the round-trip that proves the bootstrap is functionally complete, not just structurally present.

## Out of scope

- Subscription matching semantics — covered by FT-002's TC-009.
- Subscription removal / replacement — covered by `ft_002_subscription_registry.rs`.
- Outbox delivery — covered by TC-010 / TC-011.

## Formal specification

⟦Γ:Invariants⟧{
  ∀store:Store ∈ initialized_stores:
    |{sub | (sub, rdf:type, oxi:Subscription) ∈ store@subscriptions_graph}| = 2
    ∧ (DispatchAvailableId, rdf:type, oxi:Subscription) ∈ store@subscriptions_graph
    ∧ (DispatchCompletedId, rdf:type, oxi:Subscription) ∈ store@subscriptions_graph
    ∧ ∀sub ∈ {DispatchAvailableId, DispatchCompletedId}:
        ∃q:string: (sub, oxi:selectQuery, q) ∈ store@subscriptions_graph ∧ q ≠ ""
        ∧ ¬∃q': (sub, oxi:askQuery, q') ∈ store@subscriptions_graph
        ∧ (sub, oxi:mode, "inline") ∈ store@subscriptions_graph
  ∀store:Store ∈ post_implement_stores:
    |{ev | (ev, rdf:type, oxi:Event) ∈ store}| ≥ 2
    ∧ DispatchAvailableId ∈ {sub | (ev, oxi:matchedSubscription, sub) ∈ store}
    ∧ DispatchCompletedId ∈ {sub | (ev, oxi:matchedSubscription, sub) ∈ store}
}

⟦Ε⟧⟨δ≜0.95;φ≜90;τ≜◊⁺⟩