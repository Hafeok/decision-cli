---
id: TC-109
title: Verifier dispatch with confidence 0.6 escalates to standard-reasoning-frontier with bidirectional linkage
type: exit-criteria
status: passing
validates:
  features:
  - FT-062
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test escalate_confidence_07
runner-timeout: 180
last-run: 2026-05-23T18:00:12.691374679+00:00
last-run-duration: 0.2s
---

## Description

Scenario (PRD §11.2 bullet 2): a `verifier` dispatch whose worker returns `confidence = 0.6` produces a second session with the `standard-reasoning-frontier` capability binding; the two sessions are linked via `escalated_to`/`escalated_from`; the escalation reason is `confidence_below_0.7`. The escalated session's bundle is enriched with the prior attempt per [ADR-034](ADR-034).

The runner is `cargo-test` with a stubbed verifier worker that returns `confidence = 0.6` on the first call and a high-confidence verdict on the second.

Acceptance:

1. **Setup.** Seed the catalog. Stub the worker:
   - Call 1: `{verdict: "amendment-required", confidence: 0.6, rationale: "…", violates: ["TC-029"]}`.
   - Call 2 (escalated): `{verdict: "approved", confidence: 0.92, rationale: "agreed with tier-1 verdict", violates: []}`.
2. **Dispatch.** `dispatch_role(graph, "verifier", bundle)` with `stakes = "routine"`.
3. **Two sessions.** Query for sessions in this dispatch group; assert exactly 2 rows.
4. **Order and capabilities.** Session 1 has `capability.capability_id = "code-writer"`. Session 2 has `capability.capability_id = "standard-reasoning-frontier"`.
5. **Bidirectional linkage.** Query `?S1 dec:escalated_to ?S2 ; ?S2 dec:escalated_from ?S1`. Assert one row pointing S1 → S2.
6. **Escalation reason.** `?S2 dec:escalation_reason ?r` returns `?r = "confidence_below_0.7"`. S1's `escalation_reason` is absent.
7. **Bundle enrichment.** Inspect the bundle hash on S2's dispatch event payload. Assert it differs from S1's bundle hash. Fetch the enriched bundle's markdown; assert it contains a `## Prior attempt (tier 1, capability code-writer, model qwen3-coder-30b-a3b-instruct)` section, the prior verdict's text, and the literal framing string `"agree, refute, or refine"` from [ADR-034](ADR-034).
8. **Chain helper.** `escalation_chain(S2)` returns `[S1, S2]` in order. Same call with `S1` returns the same list.
9. **Telemetry.** S2's telemetry records `attempt_index = 2`, `escalated_from = S1`, `prior_attempt_capability = code-writer`.
10. **No third session.** S2's confidence (0.92) is above all thresholds; assert no further dispatch occurs.

⟦Σ:Types⟧{
  Chain ≜ List SessionRecord with bidirectional edges
}

⟦Γ:Invariants⟧{
  S2.confidence ≥ 0.7 ⇒ |chain| = 2
  S2.escalation_reason = "confidence_below_0.7"
  S2.bundle.hash ≠ S1.bundle.hash
  S2.bundle.markdown contains "## Prior attempt"
}