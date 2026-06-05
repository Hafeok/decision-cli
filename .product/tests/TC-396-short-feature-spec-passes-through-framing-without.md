---
id: TC-396
title: short feature_spec passes through framing without truncation
type: scenario
status: passing
validates:
  features:
  - FT-163
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_163_short_spec_passes_through_unchanged
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T10:33:54.266178533+00:00
last-run-duration: 0.2s
---

## Description

Scenario test for [FT-163](FT-163) §Invariants — small specs pass through framing unchanged. Pins the truncation function's no-op path.

## Assertions

For an input shorter than `MAX_FRAMING_CHARS`:
1. `truncate_for_framing(input, MAX_FRAMING_CHARS) == input` (byte-identical).
2. The output does **not** contain the `[spec truncated for cell framing]` witness suffix.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_163_short_spec_passes_through_unchanged`.