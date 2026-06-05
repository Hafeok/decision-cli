---
id: TC-402
title: malformed or out-of-range override values degrade to None without erroring dispatch
type: scenario
status: passing
validates:
  features:
  - FT-164
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_164_override_malformed_or_oob_degrades_to_none
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T10:54:43.521439719+00:00
last-run-duration: 0.3s
---

## Description

Scenario test for [FT-164](FT-164) §Error handling — the override helper is defensive. Dispatch never errors over misconfiguration; bad config silently degrades to the const default.

## Assertions

1. Garbage TOML body → helper returns `None`.
2. Negative integer in `max_turns` → fails u32 conversion, returns `None`.
3. String value where integer is expected → returns `None`.

In all three cases the dispatch path uses the const default instead of erroring.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_164_override_malformed_or_oob_degrades_to_none`.