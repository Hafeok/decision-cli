---
id: TC-400
title: absent or missing override returns None so caller falls back to default cap
type: scenario
status: passing
validates:
  features:
  - FT-164
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_164_override_absent_returns_none_so_caller_falls_back_to_default
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T10:54:43.521439719+00:00
last-run-duration: 0.2s
---

## Description

Scenario test for [FT-164](FT-164) §Behaviour fallback — `read_max_turns_for_task_type` returns `None` for every absent-config case, so the caller falls back to the const default.

## Assertions

1. Tempdir with no `.dec/task-types.toml` → helper returns `None`.
2. File present but no `[task_types.<name>]` table → helper returns `None`.

Pins the "config absence is the no-op" invariant from the spec.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_164_override_absent_returns_none_so_caller_falls_back_to_default`.