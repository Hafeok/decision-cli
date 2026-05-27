---
id: TC-187
title: 'Runner splits defect feedback by verdict: rejected steps target implementer, amendment-required target verifier'
type: exit-criteria
status: passing
validates:
  features:
  - FT-108
  adrs:
  - ADR-026
phase: 3
runner: cargo-test
runner-args: tc_187_runner_splits_feedback_by_verdict
runner-timeout: 60
last-run: 2026-05-27T09:13:24.586155434+00:00
last-run-duration: 61.2s
---

## Claim

When `core::verify::runner::run_graph` finishes a verification run, the emitted `dec:Feedback` artifacts for failing evidence-bearing steps carry `dec:targetRole = "implementer"` iff the graph's verdict is `rejected` (evidence regression on at least one TC), and `dec:targetRole = "verifier"` iff the verdict is `amendment-required` (graph problem). `approved` verdicts emit no defect feedback.

## Scenarios

### Setup

- Two minimal verification graphs, each with one evidence-bearing shell-command step:
  - `VG-T187a`: command is `false` (exits 1 reliably) — fail produces evidence regression → `rejected`.
  - `VG-T187b`: command is `bash -c 'exit 0'` (passes) AND has a sibling non-evidence-bearing step that fails to setup — `amendment-required`.
- Both verify the same fixture feature and run in the same env.

### Test

Call `run_graph` for each. Inspect the emitted feedback artifacts in the orchestration store:

1. For `VG-T187a` (rejected): every emitted feedback has `dec:targetRole = "implementer"`, `dec:class = "defect"`, `dec:lifecycleState = "produced"`.
2. For `VG-T187b` (amendment-required): every emitted feedback has `dec:targetRole = "verifier"`.
3. No feedback is emitted for `approved` verdicts.

### Boundary

- A step with `outcome = unrunnable` continues to emit `class = "gap"` to `spec-author` regardless of verdict (this slice does not modify the gap path).