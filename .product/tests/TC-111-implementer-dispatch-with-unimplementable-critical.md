---
id: TC-111
title: Implementer dispatch with unimplementable-critical feedback escalates to code-writer-heavy
type: scenario
status: passing
validates:
  features:
  - FT-062
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test escalate_unimplementable_critical
runner-timeout: 180
last-run: 2026-05-23T18:00:12.691374679+00:00
last-run-duration: 0.2s
---

## Description

Scenario (PRD §11.2 bullet 6): an `implementer` dispatch whose worker emits a `Feedback` artifact with `class = unimplementable` and `severity = critical` escalates to `code-writer-heavy` (per the `feedback_unimplementable_critical` trigger in the implementer's seed binding).

The runner is `cargo-test` with the implementer worker stubbed to emit the critical feedback on call 1 and return a successful `CodeChange` on call 2.

Acceptance:

1. **Setup.** Seed catalog. Compose a bundle with `stakes = "routine"`. Stub the implementer worker:
   - Call 1: emit feedback `{class: "unimplementable", severity: "critical", body: "ontology change required"}` and return a placeholder/failed `CodeChange` with `applied = false`.
   - Call 2 (after escalation): return a successful `CodeChange { applied: true, files: [...] }`.
2. **Dispatch.** `dispatch_role(graph, "implementer", bundle)`.
3. **Two sessions.** Assert exactly 2 sessions in the dispatch group.
4. **Capability progression.** S1.capability = `code-writer` (qwen3-coder-30b). S2.capability = `code-writer-heavy` (devstral-2-123b).
5. **Feedback recorded on S1.** The feedback artifact's `dec:produced_by` points at S1. The artifact is in the graph and visible to feedback routing per [ADR-026](ADR-026) (escalation does *not* consume / close the feedback; routing remains independent).
6. **Escalation reason.** `?S2 dec:escalation_reason "feedback_unimplementable_critical"`.
7. **Bundle enrichment.** S2's bundle includes the S1 `CodeChange` summary in its `Prior attempt` section, plus the feedback artifact's body inline (so the heavier model sees the obstacle the previous tier hit).
8. **No further escalation.** S2 succeeds; assert no S3.
9. **Both endpoints are Scaleway.** S1 and S2 both have `endpoint = scaleway` (this scenario stays inside the Scaleway tier ladder — no Anthropic path triggered).
10. **Signal collection** for the would-be S3 step: `signals.feedback_classes = []` (S2's feedback list is empty) and `signals.confidence` is `None` (CodeChange has no confidence field) and `signals.stakes = Routine` — none of the deep-reasoning step's triggers fire, so termination.

⟦Σ:Types⟧{
  Feedback ≜ ⟨class:FeedbackClass, severity:Severity, body:String⟩
}

⟦Γ:Invariants⟧{
  feedback_classes.contains(unimplementable) ∧ feedback_critical ⇒ escalates_to(code-writer-heavy)
  feedback_artifact.produced_by = S1 ∧ escalation_chain(S1) = [S1, S2]
}