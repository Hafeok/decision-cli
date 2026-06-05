---
id: TC-395
title: MAX_FRAMING_CHARS constant pinned at 50_000
type: exit-criteria
status: passing
validates:
  features:
  - FT-163
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_163_max_framing_chars_is_50k
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T10:33:54.266178533+00:00
last-run-duration: 0.2s
---

## Description

Exit-criteria test for [FT-163](FT-163) — pins the `MAX_FRAMING_CHARS` constant at the value the slice ships (`50_000`). Any change to the cap becomes explicit (must update the test) instead of silent drift.

## Assertion

`MAX_FRAMING_CHARS == 50_000`.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_163_max_framing_chars_is_50k`.