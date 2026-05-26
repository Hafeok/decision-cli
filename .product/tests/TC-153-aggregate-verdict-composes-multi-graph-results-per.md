---
id: TC-153
title: aggregate_verdict() composes multi-graph results per ADR-028 with rejection dominance and gap reporting
type: exit-criteria
status: passing
validates:
  features:
  - FT-097
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_153_aggregate_verdict_composes_multi_graph_results_per
runner-timeout: 120
last-run: 2026-05-26T13:12:58.060493885+00:00
last-run-duration: 0.4s
---

## Claim

The pure function `core::verify::aggregate::aggregate_verdict(target, &[VerificationGraphResult]) -> AggregateVerdict` composes a set of per-graph results into a single verdict according to ADR-028 §Multi-graph aggregation, with the tie-breaking made explicit in FT-097, and surfaces uncovered TCs in `coverage_gaps`.

## Scenarios

For `AggregationTarget::Feature(FT-FIXTURE)` with `FT-FIXTURE.tests = [TC-A, TC-B]`:

| # | Per-result `(verdict, covers)` tuples | Expected aggregate verdict | Expected `coverage_gaps` |
|---|--------------------------------------|----------------------------|--------------------------|
| 1 | `[]` (empty) | `rejected` rationale `"no verification graph result covers <FT-FIXTURE>"` | `[TC-A, TC-B]` |
| 2 | `[(approved, {TC-A}), (approved, {TC-B})]` | `approved` | `[]` |
| 3 | `[(approved, {TC-A})]` (TC-B uncovered) | `rejected` (the empty-set row applies per-TC for TC-B; rejection dominates) | `[TC-B]` |
| 4 | `[(approved, {TC-A, TC-B}), (rejected, {TC-A})]` | `rejected` — rejection dominates | `[]` |
| 5 | `[(approved, {TC-A, TC-B}), (amendment-required, {TC-A})]` | `amendment-required` — no `rejected` present, mix of approved + amendment-required | `[]` |
| 6 | `[(amendment-required, {TC-A}), (amendment-required, {TC-B})]` | `amendment-required` | `[]` |
| 7 | `[(approved, {TC-A, TC-B}), (approved, {TC-A, TC-B})]` (redundant cover) | `approved` | `[]` |

For `AggregationTarget::Tc(TC-A)`:

| # | Per-result `(verdict, covers)` tuples | Expected aggregate verdict | Expected `coverage_gaps` |
|---|--------------------------------------|----------------------------|--------------------------|
| 8 | `[(approved, {TC-A})]` | `approved` | `[]` |
| 9 | `[(approved, {TC-A}), (rejected, {TC-A})]` | `rejected` | `[]` |
| 10 | `[(approved, {TC-B})]` (TC-A not covered) | `rejected` rationale `"no verification graph result covers <TC-A>"` | `[TC-A]` |

For each scenario the test must also assert `contributing_results` lists exactly the `VGR` IRIs that drove the verdict (i.e. for `approved` aggregates, all that covered; for `rejected`, those that contributed `rejected` outcomes; for empty sets, an empty list).

## Runner

`cargo test` against table-driven tests in `crates/decision-cli/src/core/verify/aggregate_tests.rs`. The test constructs `VerificationGraphResult` values directly in memory (no `StreamWriter`); the function under test is pure. Each row is a separate `#[test]` or a parameterised harness — the failure message must name the row index.

## Non-goals

- Persistence of the aggregate verdict as its own artifact (out of slice; the function returns a value, the CLI/subscription decides where to use it).
- Cross-environment de-duplication policy (the function takes whatever results the caller hands it).
- The `core::verify::single_graph_verdict` per-graph rule (TC-152 covers that).