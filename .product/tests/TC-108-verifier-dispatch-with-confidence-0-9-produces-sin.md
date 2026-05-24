---
id: TC-108
title: Verifier dispatch with confidence 0.9 produces single session, no escalation
type: scenario
status: passing
validates:
  features:
  - FT-061
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test dispatch_no_escalation
runner-timeout: 180
last-run: 2026-05-24T19:14:20.003667351+00:00
last-run-duration: 0.2s
---

## Description

Scenario (PRD §11.2 bullet 1): a `verifier` dispatch with `stakes = routine` whose worker returns a `VerificationVerdict` with `confidence = 0.9` produces exactly one session with the `code-writer` capability binding and no escalation.

This is the *default-capability happy path* — it verifies that [FT-061](FT-061) ships without invoking any escalation logic from [FT-062](FT-062) (i.e. when no escalation trigger fires).

The runner is `cargo-test` with a stubbed verifier worker that returns a canned high-confidence verdict.

Acceptance:

1. **Setup.** Initialise the orchestration store with the PRD-seeded catalog ([FT-058](FT-058)). Stub the verifier worker (`workers/verifier` test fixture) to return `{verdict: "approved", confidence: 0.9, rationale: "…", violates: []}` regardless of bundle content.
2. **Dispatch.** Call `core::dispatcher::dispatch_role(graph, "verifier", bundle)` with a freshly composed bundle whose `stakes = "routine"` (default ladder, since focal is a normal feature_spec).
3. **Single session.** Query `SELECT ?s WHERE { ?s a dec:SessionRecord ; dec:dispatch_group <…> }`. Assert exactly one row.
4. **Capability pin.** Assert the session's `dec:capability` resolves to a `dec:Capability` with `capability_id = "code-writer"` and `version = 1`.
5. **No escalation edges.** Assert `?session dec:escalated_from ?prior` returns zero rows and `?session dec:escalated_to ?next` returns zero rows.
6. **Chain helper agrees.** `core::graph::session::escalation_chain(session_id)` returns a list of length 1 containing only the dispatched session.
7. **Telemetry.** The session's telemetry block records `attempt_index = 1`, `escalation_exhausted = false`, no `escalation_reason`.
8. **Cost.** `aggregate_chain_cost(chain)` matches the single attempt's cost (sanity check that nothing summed twice).

⟦Σ:Types⟧{
  Chain ≜ List SessionRecord
}

⟦Γ:Invariants⟧{
  confidence_above_threshold(verdict) ⇒ |chain| = 1
  |chain| = 1 ⇒ root.escalated_from = ⊥ ∧ root.escalated_to = ⊥
}