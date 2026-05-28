---
id: TC-185
title: Accepting a re-authored proposal transitions consumed feedback from produced to addressed
type: exit-criteria
status: failing
validates:
  features:
  - FT-107
  adrs:
  - ADR-024
  - ADR-026
phase: 3
runner: cargo-test
runner-args: tc_185_accept_transitions_consumed_feedback
runner-timeout: 60
last-run: 2026-05-28T08:49:13.813112667+00:00
last-run-duration: 0.4s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

When `run_accept` (CLI `--accept` path or MCP `dec_verify_graph_accept`) persists a re-authored proposal whose worker output cites feedback IRIs in its `addressed_feedback_iris` field, each cited `dec:Feedback` transitions from `lifecycleState = "produced"` to `lifecycleState = "addressed"` in the same commit, with the newly-minted `VG-NNN` as the `dec:addressedBy` artifact.

## Scenarios

### Setup

- A `(FT-T3, ENV-T3)` pair with one defect feedback `FB-T3`, `lifecycleState = "produced"`.
- A stubbed worker (FT-080 instrumentation pattern) that returns a `New` proposal naming `FB-T3` in its `addressed_feedback_iris`.

### Test

Call `run_generate` with `mode = Accept`. After the call returns:

1. The new graph `VG-Nnew` exists on disk.
2. `FB-T3` in the orchestration store now has `lifecycleState = "addressed"`.
3. `FB-T3` carries `dec:addressedBy <iri-of-VG-Nnew>`.
4. The transition is atomic — `FB-T3` lifecycle and `VG-Nnew` persistence land in the same `StreamWriter::commit`.

### Boundary

- A feedback not cited by the worker stays `produced` (no over-eager close).
- A feedback cited that does not exist in the store causes the accept to fail (referential integrity).