---
id: TC-401
title: per-task-type override returns the configured cap from task-types.toml
type: scenario
status: passing
validates:
  features:
  - FT-164
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_164_override_returns_configured_cap_per_task_type
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T10:54:43.521439719+00:00
last-run-duration: 0.2s
---

## Description

Scenario test for [FT-164](FT-164) §Behaviour override — catalog takes precedence. Different task types can carry different caps; unknown task types fall back to default.

## Assertions

With a `.dec/task-types.toml` carrying:
```toml
[task_types.add-artifact-type]
max_turns = 60
[task_types.add-judge-worker]
max_turns = 12
```

1. `read_max_turns_for_task_type(_, "add-artifact-type") == Some(60)` — override returned.
2. `read_max_turns_for_task_type(_, "add-judge-worker") == Some(12)` — per-task-type independence.
3. `read_max_turns_for_task_type(_, "extend-planner-classifier") == None` — unknown task type falls back to default at the call site.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_164_override_returns_configured_cap_per_task_type`.