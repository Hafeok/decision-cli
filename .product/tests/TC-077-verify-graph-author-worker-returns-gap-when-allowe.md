---
id: TC-077
title: verify-graph-author worker returns Gap when allowed_ops are insufficient
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: pytest
runner-args: workers/verify-graph-author/tests/test_tc_077_gap.py
runner-timeout: 120
last-run: 2026-05-24T19:14:09.038273853+00:00
last-run-duration: 0.4s
---

## Premise

A bundle for `FT-R` is constructed with TCs `[T1]`, target env `ENV-prod` whose `allowed_ops = ["http-readonly"]`. TC `T1`'s body requires asserting a side-effect of a POST (which would need `http-mutating`, not in `allowed_ops`). The mocked Claude returns `GraphProposal::Gap { uncovered_tcs: ["T1"], reason: "TC requires http-mutating but target environment allows only http-readonly" }`.

## Acceptance Criteria

- The worker exits 0 (Gap is a valid outcome, not a fault).
- stdout's `GraphProposal.kind == "gap"`.
- `proposal.gap.uncovered_tcs == ["T1"]`.
- `proposal.gap.reason` is non-empty and mentions the ops mismatch.
- The worker does **not** invent a synthetic step using `http-readonly` to fake coverage.

## Notes

Validates the worker prefers honest `Gap` over invalid `New`. This is the property that lets the chain gate trust worker output downstream — a `New` proposal is by construction a graph whose ops fit the env.