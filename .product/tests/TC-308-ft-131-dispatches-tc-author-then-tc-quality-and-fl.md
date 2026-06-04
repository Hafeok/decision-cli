---
id: TC-308
title: FT-131 dispatches tc-author then tc-quality and flips tcs_ready on accepted verdict
type: scenario
status: passing
validates:
  features:
  - FT-131
  adrs:
  - ADR-076
  - ADR-073
  - ADR-075
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_131_dispatch_tc_chain
runner-timeout: 180
observes:
- exit-code
- graph
last-run: 2026-06-04T09:35:01.130813997+00:00
last-run-duration: 0.1s
---

## Purpose

Validates FT-131 (FeatureReadyPlanner) end-to-end on the TC arm. When `tcs_linked` is below the floor, the planner dispatches tc-author, awaits the dispatch terminal state, dispatches tc-quality on the resulting TcProposal, and — per ADR-075 auto-accept — the harness flips `tcs_ready` to true when the verdict is `approved`. This closes the loop wired by ADR-073 (worker shape) and ADR-075 (acceptance routing).

## Acceptance

- The recorded dispatch sequence (in order) is `[tc-author, tc-quality]` against the same feature_spec IRI.
- After tc-quality returns an `approved` verdict, the orchestration store shows `tcs_ready=true` for the feature.
- After re-running `planner.classify(...)` on the updated store, the resulting Action is no longer `DispatchTcAuthor` (the arm is satisfied).
- The flip is performed by the harness's auto-accept handler, not by the planner directly (assert via state-change provenance).
- The test exits with status 0.

## Inputs

A mocked dispatch harness (extending the FT-119 test scaffolding) that records each dispatch and returns canned terminal states: tc-author terminal with a well-formed TcProposal, tc-quality terminal with an `approved` QualityVerdict. A synthetic orchestration store is seeded with a feature_spec lacking TCs.

## Out of scope

- The VG arm (covered by TC-309).
- Cycle detection on repeated rejections (covered by TC-310).
- No-author opt-out behaviour (covered by TC-311).