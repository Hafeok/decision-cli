---
id: TC-310
title: FT-131 PAT-002 cycle detection catches author-judge oscillation with period-N Stuck before max_iter
type: scenario
status: passing
validates:
  features:
  - FT-131
  adrs:
  - ADR-076
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_131_cycle_detection
runner-timeout: 120
observes:
- exit-code
- stdout
last-run: 2026-06-04T09:34:27.854942760+00:00
last-run-duration: 0.1s
---

## Purpose

Validates FT-131 (FeatureReadyPlanner) against ADR-076's PAT-002 cycle-detection invariant. When author and judge oscillate (author writes a TcProposal → judge rejects → re-author → judge rejects, repeating), the planner must detect the period-N cycle and reach `Action::Stuck` with a cycle-naming reason BEFORE the iteration counter reaches `max_iter`. This prevents runaway dispatch loops and keeps the readiness goal terminating.

## Acceptance

- Under stub-harness-driven oscillation, the planner reaches `Action::Stuck` strictly before iteration count == `max_iter`.
- The Stuck reason contains the substring `"cycle:"` followed by an integer N (number of rounds) and the offending arm name (e.g. `"cycle: 3 rounds on tcs"`).
- Once Stuck is returned, no further dispatch is recorded by the stub harness.
- The cycle is detected for both period-2 (immediate alternation) and period-3 (a→b→c→a) variants.
- The test exits with status 0.

## Inputs

A stub dispatch harness configured to return: tc-author terminal with TcProposal v1, tc-quality terminal with `rejected` verdict, tc-author terminal with TcProposal v1 (same proposal, by content hash), tc-quality terminal with `rejected` again, repeating. The planner is run with a `max_iter` of e.g. 10; the test asserts Stuck arrives by iteration 5 or earlier.

## Out of scope

- Non-cyclic Stuck reasons (covered by TC-307).
- Successful unblock paths (covered by TC-308 / TC-309).