---
id: TC-253
title: Goal::DefReady parses and the FeatureReadyPlanner is registered for (Feature, DefReady)
type: scenario
status: passing
validates:
  features:
  - FT-119
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::ft_119_drive_def_ready::registry_tests::tc_253
runner-timeout: 120
observes:
- exit-code
- stdout
last-run: 2026-06-02T12:26:35.495294390+00:00
last-run-duration: 0.2s
---

## Claim

`core::drive::goal::Goal::parse("def-ready")` returns `Ok(Goal::DefReady)`,
`Display` round-trips to `"def-ready"`, and
`planner_for(ArtifactKind::Feature, Goal::DefReady)` returns
`Some(Box<dyn Planner>)` whose concrete type is
`FeatureReadyPlanner<ProductionInspector>`. The CLI verb
`dec drive def-ready FT-XXX` accepts the goal without ambiguity against the
existing FT-110 verbs (`ship`, `verify`, `accept`, `cover`, `approve`).

## Scenarios

### Happy paths

| Input | Expected |
|---|---|
| `Goal::parse("def-ready")` | `Ok(Goal::DefReady)` |
| `format!("{}", Goal::DefReady)` | `"def-ready"` |
| `planner_for(Feature, DefReady)` | `Some(_)` |
| `planner_for(TestCriterion, DefReady)` | `None` |
| `dec drive def-ready FT-001` (parse only) | accepted; `goal == DefReady`, `artifact.kind == Feature` |

### Rejection paths

| Input | Why |
|---|---|
| `Goal::parse("defready")` (no hyphen) | malformed |
| `Goal::parse("ready")` | not a registered goal verb |
| `dec drive def-ready ADR-001` | DefReady has no planner for ArtifactKind::Adr; error mentions registered combinations |

### Boundary

- `Goal::parse` must remain case-sensitive; `Def-Ready` and `DEF-READY` are
  rejected. This pins consistency with the existing `ship` / `verify` parsing
  rules in FT-110.

## Notes

This TC is the registry-presence backstop. If `FeatureReadyPlanner` is
removed from `core::drive::registry::planner_for`, the
`planner_for(Feature, DefReady)` assertion fails fast — no integration test
needs to spin up the store to detect the regression.