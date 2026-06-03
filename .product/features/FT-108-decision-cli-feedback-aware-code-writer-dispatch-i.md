---
id: FT-108
title: 'decision-cli: Feedback-aware code-writer dispatch — implementer can consume runtime defects'
phase: 3
status: complete
depends-on:
- FT-011
- FT-013
- FT-107
adrs:
- ADR-022
- ADR-024
- ADR-026
- ADR-031
tests:
- TC-187
- TC-188
- TC-189
- TC-190
domains: []
domains-acknowledged: {}
---

## Description

Close the OTHER half of the verify → re-fix loop. [FT-107](FT-107) gave the verify-graph-author worker a way to consume runtime defect feedback and re-author broken graphs; this slice gives the **code-writer (implementer)** worker the equivalent path so test failures actually drive code changes.

Today the runner emits one `dec:Feedback` per failing evidence-bearing step, with `class = "defect"` and `targetRole = "verifier"` regardless of whether the failure means "the graph is wrong" or "the code under test is wrong". The verdict aggregator distinguishes `amendment-required` (graph problem) from `rejected` (evidence regression — actual code failure), but that distinction isn't carried into routing. So every `cargo test` panic in a verification run goes to the verify-graph-author's inbox where it has no business being.

Meanwhile, `DispatchPayloadJson` (the bundle the code-writer worker reads) has no `defect_feedback` field. Even if feedback were correctly targeted at the implementer, `dec implement FT-XXX` wouldn't ship it into the worker. And `dec implement` itself short-circuits on `feature.status = complete`, so a "complete" feature with outstanding test failures can't be re-implemented.

One subcommand → one slice — no new subcommand. Existing `dec implement` handler, existing runner feedback emission, existing code-writer worker; this slice modifies the three to close the implementer loop.

## Functional Specification

### Inputs

#### 1. Verdict-aware feedback routing (orchestrator side)

`core::verify::runner::feedback::emit_feedback_for_failures` currently emits with `target_role = FeedbackClass::Defect.default_target_role()` — which is hardcoded to `"verifier"`. Refactor so emission consults the per-graph **verdict** computed by the aggregator:

- `amendment-required` → `target_role = "verifier"` (graph is at fault; verify-graph-author re-authors).
- `rejected` → `target_role = "implementer"` (code regression; code-writer re-implements).
- `approved` → no defect feedback emitted (already the case).

This is a single conditional in the emitter. The runner already aggregates the verdict before phase 5 (`derive_verdict` in `runner::mod`), so the value is available at emission time without re-querying the store.

Implementation note: the existing `Disposition` enum (`FeedbackClass::default_target_role`) is the *class default*, not the *per-emission target*. The runner has always been allowed to override per-emission; we just hadn't been. This slice exercises that override path.

#### 2. Bundle assembler (implementer side)

`features/implement/worker.rs::DispatchPayloadJson` gains:

```rust
pub struct DispatchPayloadJson {
    // existing fields ...
    pub defect_feedback: Vec<DefectFeedbackRecord>,
}
```

The `DefectFeedbackRecord` shape is identical to [FT-107](FT-107)'s — we share the type rather than mint a parallel one. `features/implement/lifecycle.rs::build_dispatch_payload` populates the field by:

1. Loading the feature's TC short ids via `resolve_feature_tcs_short`.
2. Calling a new `defect_feedback_for_implementer(workdir, &tc_iris)` reader: every `dec:Feedback` with `class=defect`, `targetRole=implementer`, `lifecycleState=produced`, and `sourceArtifact ∈ tc_iris`.

The bundle_hash recomputes over the enriched payload (same pattern as FT-107).

#### 3. Code-writer worker (Python)

`code_writer.models.WorkerInput` mirrors the new field as a pydantic `defect_feedback: list[DefectFeedbackRecord]`. The system prompt gets a section:

> *"If `defect_feedback` is non-empty, prior verification runs found that this feature's tests fail at runtime. Read each entry's `evidence` — it carries the runner's diagnostic. Your code change should fix the underlying problem so the cited TCs pass. Cite the feedback IRIs you addressed in `addressed_feedback_iris` on your CodeChange."*

`CodeChangeJson` (the worker's reply) gains `addressed_feedback_iris: Vec<String>`.

The schema-level enforcement from FT-107 (Pydantic `min_items: 1` + enum closure when bundle feedback is non-empty) carries over.

#### 4. Dispatch gate

`dec implement FT-XXX` currently refuses (or no-ops) when `feature.status = complete`. New rule: if **outstanding defect feedback exists** for the feature (any `produced`-state defect with `targetRole = implementer` against this feature's TCs), the gate **falls through** to dispatch the worker. The signal is "the verification run failed; we need new code", which trumps "the feature was marked complete".

This mirrors FT-107's matcher-bypass gate. After the worker writes new code and the next verify run passes, the feedback transitions to `addressed` and the gate no longer fires, so re-running `dec implement` for the same feature without new defects is a no-op again.

### Outputs

- A `dec:CodeChange` artifact whose worker output cites the addressed feedback IRIs.
- On dispatch completion, each cited `dec:Feedback` transitions `produced → routed → received → addressed` (ADR-024 lifecycle, same walk FT-107 uses), with the `CodeChange` IRI as the `dec:addressingArtifact`.
- The orchestrator's session record links the consumed feedback via `dec:respondsToFeedback` for audit.

### State

- No on-disk schema change.
- Reads: the feedback table, feature TCs (already loaded by `resolve_feature_tcs_short`), feature spec body.
- Writes: lifecycle transitions on cited feedback (same `feedback_close::mark_addressed` helper FT-107 introduced).

### Behaviour

1. Operator runs `dec implement FT-XXX`.
2. The handler resolves the feature's TCs (unchanged).
3. **New:** the handler loads `defect_feedback_for_implementer` against those TCs.
4. **Modified gate:** if `feature.status = complete` AND no outstanding defect feedback, short-circuit (today's behaviour). If feedback exists, fall through.
5. Bundle assembled with `defect_feedback` populated. Worker dispatched.
6. Worker returns a `CodeChange` citing feedback IRIs (schema-enforced when feedback is non-empty).
7. **New:** on dispatch success, each cited feedback transitions to `addressed` with the new `CodeChange` IRI as the addressing artifact.

### Error handling

- `defect_feedback_for_implementer` query failure → log warning, treat as empty, continue (degrades to today's behaviour).
- Worker returns a `CodeChange` with empty `addressed_feedback_iris` when bundle feedback was non-empty → reject with `Error::WorkerIgnoredFeedback` (the same variant FT-107 added). Mirror-rejection for the implementer.
- Feedback IRI cited that doesn't exist in the store → fail the accept with referential-integrity error.

## Out of scope

- Re-dispatching the verifier after the implementer fixes code. The orchestrator already auto-dispatches the runner on `CodeChange` commit (FT-100). When verify next runs, it'll either pass (clearing the addressed feedback) or emit fresh defect feedback for the next loop iteration.
- Cross-feature defect feedback (a `cargo test` panic in FT-018 that's actually caused by FT-009 code). This slice scopes per-feature; cross-feature triage is a follow-up.
- ADR-026 routing-table changes. The routing default for `class=defect` stays `verifier`; this slice overrides per-emission based on verdict, which is the existing override path.

## Acceptance

1. When a verify run produces `rejected` verdict on at least one evidence-bearing step, the emitted `dec:Feedback` artifact carries `targetRole = "implementer"` (not `"verifier"`).
2. When `dec implement FT-XXX` runs and the orchestration store has outstanding `produced`-state implementer-targeted defect feedback for FT-XXX's TCs, the `DispatchPayloadJson` ships a non-empty `defect_feedback` array.
3. When the code-writer worker returns a `CodeChange` citing one or more feedback IRIs from its bundle, each cited feedback transitions to `lifecycleState = "addressed"` with the `CodeChange` IRI as `dec:addressingArtifact` in the same dispatch.
4. `dec implement FT-XXX` dispatches the worker even when `feature.status = complete` IF outstanding implementer-targeted defect feedback exists for that feature. With no outstanding feedback AND `feature.status = complete`, the gate short-circuits as today.

## Notes

This slice is the symmetric closer of [FT-107](FT-107). After it lands, the verify → re-fix → re-verify loop is closed for both directions: graph-side (verify-graph-author consumes graph-defect feedback) and code-side (code-writer consumes code-defect feedback).
