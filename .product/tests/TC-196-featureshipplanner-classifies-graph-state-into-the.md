---
id: TC-196
title: FeatureShipPlanner classifies graph state into the right Action across the verdict + feedback matrix
type: exit-criteria
status: passing
validates:
  features:
  - FT-110
  adrs: []
phase: 3
runner: cargo-test
runner-args: tc_196_feature_ship_planner_state_table
runner-timeout: 60
last-run: 2026-05-27T11:30:56.738500042+00:00
last-run-duration: 0.9s
---

## Claim

`FeatureShipPlanner::plan` returns the right `Action` for every cell in the classification table:

| aggregate verdict | implementer-open | verifier-open | expected `Action` |
|---|---|---|---|
| `Approved` | any | any | `Done` |
| any | `> 0` | any | `DispatchImplementer` |
| any | `0` | `> 0` | `DispatchVerifyGraphAuthor` |
| `NeverRun` | `0` | `0` | `DispatchVerifier` |
| `Rejected` or `AmendmentRequired` | `0` | `0` | `Stuck` |

`Done` and `Stuck` are terminal. The three `Dispatch*` variants carry the right `feature_id` and (where applicable) `env_id`.

## Scenarios

### Setup

Stub the `PlanContext`'s read primitives so each test row can inject the (verdict, implementer-feedback-count, verifier-feedback-count) tuple directly without seeding live store state. The test fixtures don't need to walk the real verify pipeline — they just need the planner to read the values the harness asserts.

### Test

For each row in the table:
1. Build a fixture `PlanContext` returning the row's tuple.
2. Call `FeatureShipPlanner::plan` with `ArtifactRef { kind: Feature, short_id: "FT-T196" }`.
3. Assert the returned `Action` matches the expected variant.
4. For `Dispatch*` actions, assert `feature_id == "FT-T196"`.

### Boundary

- The `Done` row with `implementer-open > 0` is reachable in steady-state only briefly (a verify run just landed `Approved` but the next emission hasn't cleaned the lingering feedback). The planner still returns `Done` — it does not preemptively re-dispatch when the goal is reached, even if feedback is technically open.