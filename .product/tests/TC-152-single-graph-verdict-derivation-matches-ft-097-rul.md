---
id: TC-152
title: Single-graph verdict derivation matches FT-097 rule for every step-outcome combination
type: scenario
status: unimplemented
validates:
  features:
  - FT-097
  adrs: []
phase: 1
---

## Claim

The pure function `core::verify::single_graph_verdict(traces: &[StepTrace], steps: &[VerificationStep]) -> (Verdict, String)` derives the per-graph verdict according to FT-097's rule for every relevant combination of step outcomes and `dec:providesEvidenceFor` annotations, and produces a rationale string of at least 20 characters that names the dominant cause.

## Scenarios

The function is deterministic and pure — every input combination has one correct output. Each row below is a separate assertion in the test:

| # | Step outcomes (ordered) | providesEvidenceFor on each step | Expected verdict | Rationale must contain |
|---|-------------------------|----------------------------------|------------------|------------------------|
| 1 | `[pass]` | `[[TC-001]]` | `approved` | "all 1 steps passed" |
| 2 | `[pass, pass, pass]` | `[[TC-001], [], [TC-002]]` | `approved` | "all 3 steps passed" |
| 3 | `[pass, fail]` | `[[TC-001], [TC-002]]` | `rejected` | "step 1" and "TC-002" |
| 4 | `[fail, pass]` | `[[], [TC-001]]` | `amendment-required` | "step 0" and "setup" or "capture" or "before" |
| 5 | `[unrunnable]` | `[[TC-001]]` | `amendment-required` | "unrunnable" |
| 6 | `[pass, unrunnable, pass]` | `[[TC-001], [TC-002], [TC-003]]` | `amendment-required` | "step 1" and "unrunnable" |
| 7 | `[fail, unrunnable]` | `[[TC-001], [TC-002]]` | `rejected` | "step 0" — fail dominates unrunnable |
| 8 | `[]` (empty graph) | `[]` | `approved` | "0 steps" — vacuous pass (degenerate; should not happen but must be defined) |

## Runner

`cargo test` against a property-style table-driven test in `crates/decision-cli/src/core/verify/aggregate_tests.rs` (sibling to `aggregate.rs`). Each row is asserted as a separate `assert_eq!` so a failure names the failing row. No fixtures beyond the in-test data structures; pure-function tests should not touch the store.

## Non-goals

- The multi-graph composition rule (TC-153 covers that).
- The runner's behaviour that populates the trace inputs (FT-098 TCs cover that).
- Rationale formatting consistency across locales — this slice ships English rationale strings only.
