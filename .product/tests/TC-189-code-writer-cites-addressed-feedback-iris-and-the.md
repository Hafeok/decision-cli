---
id: TC-189
title: Code-writer cites addressed_feedback_iris and the dispatch transitions each cited feedback to addressed
type: exit-criteria
status: failing
validates:
  features:
  - FT-108
  adrs:
  - ADR-024
  - ADR-026
phase: 3
runner: cargo-test
runner-args: tc_189_dispatch_transitions_consumed_feedback
runner-timeout: 60
last-run: 2026-05-28T08:49:15.440971467+00:00
last-run-duration: 0.5s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

When `dec implement FT-XXX` runs against a feature with outstanding implementer-targeted defect feedback, and the code-writer worker returns a `CodeChange` whose `addressed_feedback_iris` cites one or more of those feedback IRIs, each cited `dec:Feedback` transitions from `produced` to `addressed` in the same dispatch, with the new `CodeChange` IRI as `dec:addressingArtifact`.

## Scenarios

### Setup

- A feature `FT-T189` with one TC `TC-T189a` and one pre-seeded defect feedback `FB-T189` (class=defect, targetRole=implementer, lifecycleState=produced, source_artifact=TC-T189a).
- A mock code-writer worker (stub-runner pattern from FT-013) that returns a `WorkerResponseJson` with `code_change.addressed_feedback_iris = [FB-T189 iri]`.

### Test

Call `dec implement FT-T189`. Assert:

1. The dispatch completes (status=succeeded).
2. `FB-T189` in the orchestration store now has `lifecycleState = "addressed"`.
3. `FB-T189` carries `dec:addressingArtifact <CodeChange IRI>` where the CodeChange IRI matches the one in the worker response.

### Boundary

- A worker that returns a `CodeChange` with empty `addressed_feedback_iris` when the bundle's `defect_feedback` is non-empty fails the dispatch with `Error::WorkerIgnoredFeedback` (mirror of FT-107's TC-186).