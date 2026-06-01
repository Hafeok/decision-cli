---
id: TC-265
title: FT-120 FT-100 drive unblocks after orphan retraction (regression)
type: scenario
status: passing
validates:
  features:
  - FT-120
  adrs:
  - ADR-024
phase: 4
runner: bash
runner-args: tests/scripts/tc-265-ft-100-unblock-regression.sh
runner-timeout: 300
last-run: 2026-06-01T09:36:07.533566345+00:00
last-run-duration: 0.3s
---

## Description

End-to-end regression: with FT-100's 8 stale orphan defects from
VG-088/VG-155 present in the orchestration store, running the
orphan-retract sweep followed by `dec drive ship ship FT-100`
results in the drive reaching `Action::Done` rather than the
state-hash cycle / `stuck` outcome observed pre-FT-120.

## Acceptance criteria

1. **Pre-state snapshot.** The fixture (or live store at TC-run
   time) has at least 8 open defects sourced from VG-088 / VG-155
   referencing TC-162 / TC-163 / TC-164.
2. **Retract pass.** `dec _retract-orphan-defects --feature
   FT-100` reports the 8 retractions and exits 0.
3. **Open-defect count drops to zero.** Post-retraction,
   `open_defect_feedback_count` for FT-100 is 0.
4. **Drive reaches Done.** `dec drive ship ship FT-100 --bench
   BNCH-002 --max-iter 5` exits 0 with `Action::Done` in the final
   round (no `stuck`, no state-hash cycle).
5. **Defects show as `superseded` with reason.** `dec loop show
   FT-100` displays the retracted defects with
   `[superseded by topology change]`.

## Runner

`bash` script: `tests/scripts/tc-265-ft-100-unblock-regression.sh`.