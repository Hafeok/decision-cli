---
id: TC-456
title: add-artifact-type test cell is split into round_trip_test and shacl_negative_tests with narrow derive sets
type: exit-criteria
status: passing
validates:
  features: [FT-177]
  adrs: [ADR-091, ADR-080]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_177_test_cell_is_split
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T10:20:23.805264847+00:00
last-run-duration: 3.2s
---

## Purpose

FT-177: the oversized `round_trip_tests` cell (41k output tokens, 1.25M input tokens witnessed on FT-147) is split into `round_trip_test` and `shacl_negative_tests`, each with a narrow derive set and its own output file; the legacy cell is gone from the registry.

## Mechanism

`cargo test -p decision-cli ft_177_test_cell_is_split`.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.