---
id: TC-186
title: Worker returning kind=Match despite non-empty defect_feedback is rejected
type: exit-criteria
status: passing
validates:
  features:
  - FT-107
  adrs:
  - ADR-022
phase: 3
runner: cargo-test
runner-args: tc_186_worker_ignoring_feedback_is_rejected
runner-timeout: 60
last-run: 2026-05-26T18:54:31.667893283+00:00
last-run-duration: 0.7s
---

## Claim

When the bundle handed to the verify-graph-author has a non-empty `defect_feedback` array (i.e. the orchestrator deliberately bypassed the matcher to give the worker a re-authoring opportunity), a worker proposal with `kind = Match` is rejected with `Error::WorkerIgnoredFeedback`. The handler also emits a single meta-feedback artifact: `class = "defect"`, `targetRole = "verify-graph-author"`, `severity = "warning"`, evidence excerpt naming the feedback IRIs the worker failed to address.

## Scenarios

### Setup

- A `(FT-T4, ENV-T4)` pair with one defect feedback `FB-T4`, `lifecycleState = "produced"`.
- A stubbed worker that returns `kind = Match` regardless of the bundle (degenerate worker behaviour FT-107 must catch).

### Test

Call `run_generate` for `(FT-T4, ENV-T4)`. Assert:

1. The call returns `Err(Error::WorkerIgnoredFeedback { feedback_iris: [FB-T4 iri] })`.
2. Nothing is persisted: no new graph file, no lifecycle transition on `FB-T4`.
3. Exactly one new `dec:Feedback` artifact exists with `class=defect`, `targetRole=verify-graph-author`, evidence text mentioning `FB-T4`.

### Boundary

- A `kind = Match` proposal when the bundle's `defect_feedback` is EMPTY is the normal short-circuit path (TC-184) and is NOT rejected.