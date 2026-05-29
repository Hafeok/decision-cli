---
id: TC-245
title: Planner open-defect count drops to zero after auto-close runs on approved VGR
type: scenario
status: unimplemented
validates:
  features:
  - FT-116
  adrs: []
observes:
- graph
- stdout
phase: 4
runner: cargo-test
runner-args: tc_245_planner_open_count_drops_after_auto_close
runner-timeout: 30
---

## Description

End-to-end integration check between FT-116 and FT-110: the
planner's `open_defect_feedback_count` (the inspector method
the FT+Ship planner reads) must return zero after the
auto-close pass, matching the operator's intuition that "the
verifier said approved → no work pending." This is the
property whose absence caused FT-112's 30-minute phantom-work
dispatch.

## Acceptance Criteria

Cargo test using ProductionInspector against a temp store:

1. Seed: feature F with TC T-1, T-2, T-3. Graph G covers all
   three. VGR-1 for G emitted three defects (fb-1, fb-2,
   fb-3) all in `produced` state.
2. Capture `inspector.open_defect_feedback_count(F,
   "implementer")` — assert returns 3.
3. Write approved VGR-2 for G with `outcome="pass"` for all
   three TCs. Invoke `retract_stale_defects(store, VGR-2)`.
4. Capture `inspector.open_defect_feedback_count(F,
   "implementer")` again — assert returns 0.
5. Run `FeatureShipPlanner::classify(F, "BNCH-002")`. Assert
   returns `Action::Done` (verdict approved + zero open
   defects + zero open vga work = done).

This is the canonical property that ties FT-116's correctness
to FT-110's planner behaviour. If this test passes, the
phantom-work failure mode is structurally impossible.
