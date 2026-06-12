---
id: TC-471
title: Every cell's upstreams sit in strictly earlier levels — no same-level dependency edges
type: invariant
status: passing
validates:
  features: [FT-181]
  adrs: [ADR-080, ADR-091]
phase: 1
runner: cargo-test
runner-args: -p dec-harness ft_181_topo_levels_respect_derived_from
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T13:19:58.955295907+00:00
last-run-duration: 0.2s
---

## Purpose

FT-181 safety property over every registered TaskType: a cell never shares a level with any of its `derived_from` upstreams — concurrent dispatch within a level can never race a dependency.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.