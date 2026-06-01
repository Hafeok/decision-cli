---
id: TC-264
title: FT-120 planner inspect auto-retracts orphan defects in-line
type: scenario
status: unimplemented
validates:
  features:
  - FT-120
  adrs:
  - ADR-024
phase: 4
runner: cargo-test
runner-args: features::ft_120_retract_orphan_defects::tests::tc_264_planner_inspect_auto_retracts
runner-timeout: 30
---

## Description

The planner's inspector retracts orphan defects in-line before
computing `open_defect_feedback_count`, so the planner sees the
post-retraction count and routes correctly.

## Acceptance criteria

1. **Pre-inspect state.** Fixture has an FT with 3 open defects:
   1 orphaned, 2 live (source VG still covers the TC).
2. **Inspect retracts orphan.** Calling the inspector once
   transitions the 1 orphaned defect to `superseded` and reports
   `open_defect_feedback_count = 2`.
3. **Planner routing.** With `open_defect_feedback_count = 2`
   visible to the planner, the next dispatch is
   `verify-graph-author`. With `open_defect_feedback_count = 0`
   (no live defects remain), the planner advances per its FT
   planner rules rather than re-dispatching the author.
4. **Idempotency.** A second `inspect` call finds zero orphan
   candidates and writes nothing.
5. **Inspector failure isolation.** If a single orphan retraction
   raises a SHACL violation, the inspector logs and skips that one
   feedback rather than aborting the entire round.

## Runner

`cargo-test` against the new module's `tests.rs` (planner-integration
section).
