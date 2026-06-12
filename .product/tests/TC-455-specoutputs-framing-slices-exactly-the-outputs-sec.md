---
id: TC-455
title: SpecOutputs framing slices exactly the Outputs section with a warned fallback
type: invariant
status: passing
validates:
  features: [FT-177]
  adrs: [ADR-091, ADR-080]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_177_outputs_slicer
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T10:20:23.805264847+00:00
last-run-duration: 2.2s
---

## Purpose

FT-177: the `rust_struct` cell's framing is exactly the spec's `### Outputs` section (the shape it transcribes) — sliced to the next same-level heading; a body without the heading degrades to the capped full body rather than failing dispatch.

## Mechanism

`cargo test -p decision-cli ft_177_outputs_slicer`.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.