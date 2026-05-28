---
id: TC-192
title: dec loop list rolls up open/closed defect feedback per feature and sorts by open count descending
type: exit-criteria
status: failing
validates:
  features:
  - FT-109
  adrs:
  - ADR-024
phase: 3
runner: cargo-test
runner-args: tc_192_loop_list_rollup
runner-timeout: 60
last-run: 2026-05-28T08:49:17.282686518+00:00
last-run-duration: 0.5s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

`dec loop list` groups every `dec:Feedback` of class `defect` by the feature owning its `sourceArtifact` TC and returns one row per feature with the open-count, closed-count, and last-emitted timestamp. Default mode shows only features with open feedback; `--state all` includes fully-closed features; `--state closed` includes only closed-only features. Rows are sorted by open-count descending then last-emitted descending.

## Scenarios

### Setup

- Three features:
  - `FT-T192a` with 3 open + 1 addressed defect feedback (4 total).
  - `FT-T192b` with 1 open defect feedback.
  - `FT-T192c` with 2 addressed (closed) defect feedback, none open.
- All feedback owned by TCs validated by the respective features.

### Test

1. `dec loop list --format json` (default `open`):
   - Two rows: `FT-T192a` (open=3) THEN `FT-T192b` (open=1).
   - `FT-T192c` absent.
2. `dec loop list --state all --format json`:
   - Three rows in order `FT-T192a`, `FT-T192b`, `FT-T192c`.
   - Each row has `open_count`, `closed_count`, `last_emitted_at`.
3. `dec loop list --state closed --format json`:
   - Only `FT-T192c`.

### Boundary

- A feature with feedback whose `sourceArtifact` is NOT a TC (e.g. a catalog gap) does not appear in the rollup but contributes to the trailing "(N feedback artifacts not scoped to any feature)" line in text output.