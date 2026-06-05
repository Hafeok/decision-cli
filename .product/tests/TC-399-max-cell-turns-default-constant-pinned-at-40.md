---
id: TC-399
title: MAX_CELL_TURNS default constant pinned at 40
type: exit-criteria
status: passing
validates:
  features:
  - FT-164
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_164_max_cell_turns_default_is_40
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T10:54:43.521439719+00:00
last-run-duration: 0.3s
---

## Description

Exit-criteria test for [FT-164](FT-164) — pins the `MAX_CELL_TURNS` default constant at `40`. Operators see the per-release default; changes become explicit. Catalog overrides take precedence over this default at dispatch time (TC-401).

## Assertion

`MAX_CELL_TURNS == 40`.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_164_max_cell_turns_default_is_40`.