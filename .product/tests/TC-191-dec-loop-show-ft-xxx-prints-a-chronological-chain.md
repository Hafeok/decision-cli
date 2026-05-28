---
id: TC-191
title: dec loop show FT-XXX prints a chronological chain of every defect feedback for the feature's TCs with state transitions
type: exit-criteria
status: failing
validates:
  features:
  - FT-109
  adrs:
  - ADR-004
phase: 3
runner: cargo-test
runner-args: tc_191_loop_show_chronological_chain
runner-timeout: 60
last-run: 2026-05-28T08:49:21.209988411+00:00
last-run-duration: 0.4s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

`dec loop show <FT-NNN>` returns one entry per `dec:Feedback` whose `dec:sourceArtifact` is in FT-NNN's TC set, chronologically sorted by `dec:routedAt` (or `dec:sourceSession.startedAt` when not yet routed). Each entry surfaces feedback IRI, state, evidence excerpt, source session, and addressing artifact.

## Scenarios

### Setup

- Feature `FT-T191` with TCs `[TC-T191a, TC-T191b]`.
- Three feedback artifacts seeded in the orchestration store:
  - `FB-1` (produced, source_artifact=TC-T191a, routed_at unset, source_session ts=T1).
  - `FB-2` (addressed, source_artifact=TC-T191a, routed_at=T3, addressing_artifact=`VG-PATCH-1`).
  - `FB-3` (produced, source_artifact=TC-T191b, source_session ts=T2).
- One feedback `FB-other` with source_artifact=TC outside FT-T191 — must be EXCLUDED.

### Test

Run `dec loop show FT-T191 --format json`. Parse the JSON array. Assert:

1. Exactly three entries (FB-1, FB-2, FB-3); FB-other absent.
2. Sorted ascending: FB-1 (T1) → FB-3 (T2) → FB-2 (T3).
3. Each entry has fields `feedback_iri`, `state`, `evidence`, `source_session`, `source_tc`, and (where set) `addressing_artifact`.
4. FB-2's `addressing_artifact` resolves to short id `VG-PATCH-1` (not the full IRI).

### Boundary

- Calling `dec loop show FT-T191` against a fresh tree with no feedback exits 0 and prints `(no feedback for FT-T191)`.