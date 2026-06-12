---
id: TC-472
title: Concurrency config defaults to 3, honours task-types.toml, floors at sequential
type: invariant
status: passing
validates:
  features: [FT-181]
  adrs: [ADR-080, ADR-091]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_181_max_parallel_cells_config
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T13:19:58.955295907+00:00
last-run-duration: 0.9s
---

## Purpose

FT-181: `[concurrency] max_parallel_cells` in `.dec/task-types.toml` — default 3, override honoured, values < 1 floor to sequential (1 reproduces pre-FT-181 behaviour).

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.