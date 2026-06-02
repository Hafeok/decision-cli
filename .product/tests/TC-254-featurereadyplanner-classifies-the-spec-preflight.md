---
id: TC-254
title: FeatureReadyPlanner classifies the (spec, preflight, deps, tcs, vgs) state matrix into the right Action
type: scenario
status: unimplemented
validates:
  features:
  - FT-119
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::ft_119_drive_def_ready::planner::tests::tc_254
runner-timeout: 120
observes:
- exit-code
---

## Claim

`FeatureReadyPlanner::classify` returns the right `Action` for every cell in
the Definition-of-Ready classification table specified in FT-119.

| spec_complete | preflight | deps_done | tcs_linked | tcs_ok | vgs_cover | vgs_accepted | expected `Action` |
|---|---|---|---|---|---|---|---|
| true | clean | true | true | true | true | true | `Done` |
| any | warnings | any | any | any | any | any | `Stuck { reason ~= /^preflight:/ }` |
| any | any | false | any | any | any | any | `Stuck { reason ~= /^blocked: FT-/ }` |
| false | clean | true | any | any | any | any | `Stuck { reason ~= /^spec incomplete:/ }` |
| true | clean | true | false | any | any | any | `Stuck { reason ~= /^no TCs linked/ }` |
| true | clean | true | true | false | any | any | `Stuck { reason ~= /^TC quality: TC-/ }` |
| true | clean | true | true | true | false | any | `DispatchVerifyGraphAuthor { feature_id, env_id }` |
| true | clean | true | true | true | true | false | `Stuck { reason ~= /^VG pending_review: VG-/ }` |

`Done` and every `Stuck` variant are terminal. `DispatchVerifyGraphAuthor`
carries the right `feature_id` and a non-empty `env_id` resolved from
`PlanContext::default_bench`.

## Scenarios

### Setup

Build a `StubInspector` that returns the row's seven booleans via fixed
accessors plus a hand-rolled artifact-id list for the Stuck-reason rows
(e.g. `vec!["TC-T254a".into()]` for `tcs_ok=false`, `vec!["VG-T254b".into()]`
for `vgs_accepted=false`).

### Test

For each row in the table:
1. Build a stub inspector encoding the row.
2. Instantiate `FeatureReadyPlanner::new(stub)`.
3. Call `classify("FT-T254", "BNCH-002")`.
4. Assert the returned `Action` matches the expected variant.
5. For `Stuck` rows, assert the reason matches the regex; for
   `DispatchVerifyGraphAuthor`, assert `feature_id == "FT-T254"` and
   `env_id == "BNCH-002"`.

### Boundary

- The `Done` row with `tcs_linked: true` AND `tests.len() == 0` is a
  contradiction surface; the inspector enforces `tcs_linked = tests.len() > 0`
  so this row is unreachable. The unit test asserts the inspector helper
  upholds that invariant by construction (a single `debug_assert!`).
- Row priority: when multiple "stuck" conditions hold simultaneously, the
  precedence is `preflight > deps > spec > tcs_linked > tcs_ok > vgs_cover >
  vgs_accepted` (top-to-bottom of the table). A test row asserts that order
  by holding several bits false at once and inspecting which reason wins.

## Notes

This TC is the pure-classification backstop. It must run as a `#[cfg(test)]`
unit test inside the feature slice and complete in milliseconds against
stubs — no orchestration store, no SPARQL, no filesystem. The pattern is
PAT-001 verbatim; the row table is the single source of truth for what
"ready" structurally means.
