---
id: TC-405
title: cluster cell bundle caps writes at single target path
type: scenario
status: passing
validates:
  features:
  - FT-165
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_165_bundle_caps_writes_at_single_target
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T11:36:20.721291363+00:00
last-run-duration: 0.5s
---

## Description

Scenario test for [FT-165](FT-165) §Invariants single-target cap. Removes the witnessed "let me also create a helper" failure mode (a stray `product.verify` file appeared in the FT-147 sandbox on retry).

## Assertions

The bundle contains:
1. `Do not create any other files.`
2. `The target path is the ONLY file you may write.`

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_165_bundle_caps_writes_at_single_target`.