---
id: TC-404
title: cluster cell bundle forbids pasting file content into assistant message
type: scenario
status: passing
validates:
  features:
  - FT-165
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_165_bundle_forbids_pasting_content_in_text
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T11:36:20.721291363+00:00
last-run-duration: 0.4s
---

## Description

Scenario test for [FT-165](FT-165) §Invariants anti-narrate guard — the bundle explicitly forbids the "paste content in text response and call it done" failure mode that aborted ~50% of FT-147 substrate cells.

## Assertions

The bundle contains:
1. `Do not paste the file content into your assistant message text` — the anti-paste guard.
2. `Only a `write_file` tool call counts` — the explicit success criterion.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_165_bundle_forbids_pasting_content_in_text`.