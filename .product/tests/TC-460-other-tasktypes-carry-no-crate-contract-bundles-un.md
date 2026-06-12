---
id: TC-460
title: Other TaskTypes carry no crate contract — bundles unchanged outside add-artifact-type
type: invariant
status: passing
validates:
  features: [FT-178]
  adrs: [ADR-091]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_178_other_task_types_unaffected
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T10:51:13.853263711+00:00
last-run-duration: 2.1s
---

## Purpose

FT-178 boundary: TaskTypes other than add-artifact-type carry empty crate contracts and no context files — their bundles are byte-identical to pre-FT-178. Runner: `ft_178_other_task_types_unaffected`.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.