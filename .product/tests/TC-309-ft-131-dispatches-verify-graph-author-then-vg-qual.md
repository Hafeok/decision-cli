---
id: TC-309
title: FT-131 dispatches verify-graph-author then vg-quality and flips vgs_ready on accepted verdict
type: scenario
status: unimplemented
validates:
  features:
  - FT-131
  adrs:
  - ADR-076
  - ADR-073
  - ADR-075
  - ADR-030
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_131_dispatch_vg_chain
runner-timeout: 180
observes:
- exit-code
- graph
---

## Purpose

Validates FT-131 (FeatureReadyPlanner) end-to-end on the VG arm. When `vgs_cover` is false (covered TCs are not yet wired into a VerificationGraph), the planner dispatches verify-graph-author, awaits dispatch terminal, dispatches vg-quality on the resulting GraphProposal, and on `approved` verdict the harness auto-flips `vgs_ready` per ADR-075. Analogous to TC-308 but for the VG arm and its ADR-030 step-vocabulary constraints.

## Acceptance

- The recorded dispatch sequence (in order) is `[verify-graph-author, vg-quality]` against the same feature_spec IRI.
- After vg-quality returns an `approved` verdict, the orchestration store shows `vgs_ready=true` for the feature.
- After re-running `planner.classify(...)` on the updated store, the resulting Action is no longer `DispatchVerifyGraphAuthor`.
- The flip is performed by the harness's auto-accept handler.
- The test exits with status 0.

## Inputs

The same mocked dispatch harness used in TC-308, with the orchestration store seeded such that TCs exist but no VerificationGraph covers them. verify-graph-author terminal returns a well-formed `GraphProposal(kind="new")`; vg-quality terminal returns an `approved` QualityVerdict.

## Out of scope

- TC arm (covered by TC-308).
- Cycle detection (TC-310) and no-author opt-out (TC-311).
- Gap-routing path (covered by TC-297 from the worker side).

