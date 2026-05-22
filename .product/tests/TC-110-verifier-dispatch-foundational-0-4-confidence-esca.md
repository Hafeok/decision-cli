---
id: TC-110
title: Verifier dispatch foundational+0.4 confidence escalates twice into deep-reasoning
type: scenario
status: unimplemented
validates:
  features:
  - FT-062
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test escalate_foundational_chain
runner-timeout: 240
---

## Description

Scenario (PRD §11.2 bullet 3): a `verifier` dispatch with `stakes = foundational` and a worker returning `confidence = 0.4` escalates *twice* — first to `standard-reasoning-frontier` (via `stakes_elevated` / `confidence_below_0.7`, whichever fires first in the seed binding's escalation order), then to `deep-reasoning` (via `stakes_foundational` / `confidence_below_0.5`). All three sessions are linked in a chain.

The runner is `cargo-test` with the verifier worker stubbed to return `confidence = 0.4` on call 1, `confidence = 0.45` on call 2, and `confidence = 0.95` on call 3.

Acceptance:

1. **Setup.** Seed catalog. Compose a bundle whose focal artifact is a `dec:Capability` (so `default_stakes_for` returns `Foundational`). Confirm via `bundle.stakes == Foundational`.
2. **Stub worker** with the three canned responses above.
3. **Dispatch.** `dispatch_role(graph, "verifier", bundle)`.
4. **Three sessions.** Query for sessions in the dispatch group; assert exactly 3 rows.
5. **Capability progression.** S1.capability = `code-writer`, S2.capability = `standard-reasoning-frontier`, S3.capability = `deep-reasoning`. (Verifier's seed binding per PRD §6.2.)
6. **Chain linkage.** Assert `S1 → S2 → S3` via `escalated_to` and reverse via `escalated_from`. `escalation_chain(S3)` returns `[S1, S2, S3]`.
7. **Escalation reasons.** S2's reason is one of {`confidence_below_0.7`, `stakes_elevated`} (whichever fires first in the binding's first escalation step's trigger set per [ADR-034](ADR-034) — first matching step wins). S3's reason is one of {`confidence_below_0.5`, `stakes_foundational`}.
8. **Bundle enrichment.** S2's bundle hash differs from S1's. S3's bundle hash differs from S2's. S3's enriched bundle contains a `Prior attempt (tier ?, capability standard-reasoning-frontier, …)` section.
9. **Endpoint progression.** S1 + S2 are `endpoint = scaleway`. S3 is `endpoint = anthropic` (per [ADR-037](ADR-037)).
10. **Aggregate cost.** `aggregate_chain_cost(chain)` reports the sum across all three attempts, including the Anthropic premium on S3.

⟦Σ:Types⟧{
  Chain ≜ Ordered List SessionRecord
}

⟦Γ:Invariants⟧{
  stakes = foundational ∧ confidence < 0.5 ⇒ |chain| = 3
  chain[2].endpoint = anthropic
  chain[2].capability = deep-reasoning
  aggregate_chain_cost(chain).eur ≥ Σ per_attempt_cost(chain[i])
}
