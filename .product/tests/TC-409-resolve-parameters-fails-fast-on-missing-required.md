---
id: TC-409
title: resolve_parameters fails fast on missing required parameter with operator diagnostic
type: scenario
status: passing
validates:
  features:
  - FT-166
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_166_resolve_parameters_fails_fast_on_missing_required_param
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T12:27:44.473444561+00:00
last-run-duration: 0.2s
---

## Description

Scenario test for [FT-166](FT-166) §Invariants "Required parameters surface early". A required parameter (no default) with no per-feature override fails dispatch before any cell runs, with a clean operator diagnostic.

## Assertions

1. `resolve_parameters` returns `Err` when the TaskType declares a required parameter (`default: None`) and no `.dec/task-types.toml` value supplies it.
2. The error message names the missing parameter explicitly (`requires parameter \`artifact_name\``).
3. The error message surfaces the TOML location to populate (`[parameters."FT-Tmissing"]`).

Operator gets the dispatch-fail diagnostic before any worker dispatches — saves a botched cluster run.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_166_resolve_parameters_fails_fast_on_missing_required_param`.