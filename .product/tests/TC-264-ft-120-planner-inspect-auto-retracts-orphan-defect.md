---
id: TC-264
title: FT-120 pipeline retracts orphan defects idempotently
type: scenario
status: passing
validates:
  features:
  - FT-120
  adrs:
  - ADR-024
phase: 4
runner: cargo-test
runner-args: features::ft_120_retract_orphan_defects::tests::tc_264_pipeline_retracts_orphans_idempotent
runner-timeout: 30
last-run: 2026-06-01T10:13:00.978438776+00:00
last-run-duration: 0.6s
---

## Description

The pipeline `find_orphan_defects_for_graph` + `retract_orphan` flow
correctly distinguishes orphaned defects from live ones and is
idempotent across repeated invocations.

(The originally-spec'd planner-inspector auto-retract integration —
which would run the pipeline inline during the planner's open-defect
count — is deferred to a follow-up feature. The MVP retraction is
operator-driven via `dec _retract-orphan-defects`, validated by
TC-262 and TC-263; this TC pins the in-process contract that
operator path relies on.)

## Acceptance criteria

1. **Pre-state.** Fixture has an FT context with 2 open defects:
   1 orphaned (source VG has no step covering the TC), 1 live
   (source VG has a step that covers the TC).
2. **Query returns exactly the orphan.**
   `find_orphan_defects_for_graph` returns one row matching the
   orphan; the live defect is not returned.
3. **Retraction transitions only the orphan.**
   `retract_orphan` on the orphan transitions it to `superseded`.
   The live defect's lifecycle state is unchanged.
4. **Idempotency.** A second pass of
   `find_orphan_defects_for_graph` returns an empty list.

## Runner

`cargo-test` against the new module's `tests.rs`.