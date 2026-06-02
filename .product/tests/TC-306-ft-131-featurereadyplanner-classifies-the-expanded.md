---
id: TC-306
title: FT-131 FeatureReadyPlanner classifies the expanded eight-dimension matrix into the right Action
type: scenario
status: unimplemented
validates:
  features:
  - FT-131
  adrs:
  - ADR-076
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_131_classification_table
runner-timeout: 120
observes:
- exit-code
---

## Purpose

Validates FT-131 (FeatureReadyPlanner) against ADR-076's expanded 13-row classification table. The planner's `classify` function must return the right `Action` (Ready / DispatchTcAuthor / DispatchSpecAuthor / DispatchAdrAuthor / DispatchVerifyGraphAuthor / DispatchAcknowledgeAdrs / Stuck) for every row, with first-match-wins precedence preserved. Mirrors TC-254's shape for FT-119.

## Acceptance

- Every row of the ADR-076 13-row classification table has a corresponding test case in the table-driven test.
- For each row, `FeatureReadyPlanner::classify(StubInspector(...))` returns the `Action` declared in the row.
- Row precedence is enforced: when multiple rows could match (e.g. blocked-by-dep + missing-tcs), the earlier row wins.
- All assertions complete in under 500 ms total (no I/O, no graph store, only `StubInspector`).
- The test fails with a row-naming message when any cell deviates (e.g. `"row 7 (vgs_cover=false, vgs_pending_review=false): expected DispatchVerifyGraphAuthor, got Ready"`).

## Inputs

A `StubInspector` per row constructed via the FT-119 / FT-117 test-scaffolding pattern: per-row tuples of (tcs_linked, tcs_pending_review, tcs_rejected, vgs_cover, vgs_pending_review, vgs_rejected, blocked_by, adrs_unack, expected_action). The test iterates the fixture table and calls `planner.classify(&stub)`.

## Out of scope

- Stuck-reason string formatting (covered by TC-307).
- Dispatch sequencing and harness integration (covered by TC-308 / TC-309).

