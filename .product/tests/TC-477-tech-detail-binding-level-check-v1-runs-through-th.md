---
id: TC-477
title: Tech-detail binding-level check v1 runs through the ADR-013 contract — vacuous-pass before FT-160
type: invariant
status: passing
validates:
  features: [FT-148]
  adrs: [ADR-082, ADR-083]
phase: 1
runner: bash
runner-args: scripts/checks/tech-detail-binding-level.sh
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-12T13:34:22.523452111+00:00
last-run-duration: 0.0s
---

## Purpose

FT-148 / ADR-083 v1: the binding-level script follows the ADR-013 exit contract and vacuous-passes while forge/archetypes/ holds no contract pairs (the first lands with FT-160); it binds for real from then on.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.