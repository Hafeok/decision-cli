---
id: TC-398
title: framing cap admits the catalog largest current archetype spec without truncation
type: scenario
status: passing
validates:
  features:
  - FT-163
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_163_cap_admits_current_archetype_spec
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T10:33:54.266178533+00:00
last-run-duration: 0.2s
---

## Description

Scenario test for [FT-163](FT-163) §Description witness — guards against `MAX_FRAMING_CHARS` silently shrinking below the catalog's largest current spec. Reads the live FT-147 spec file from `.product/features/` and asserts the cap admits it without truncation.

This is the load-bearing test for FT-163's purpose: the framing fix exists *because* FT-147's spec didn't fit at the old cap. If a future change drops `MAX_FRAMING_CHARS` back to 2000 (or under 12k), this TC fails loudly with a clear remediation message.

## Assertion

`FT-147 spec char count ≤ MAX_FRAMING_CHARS`.

If the FT-147 spec is absent from the checkout, the test returns early (not a regression — the cap is what it is).

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_163_cap_admits_current_archetype_spec`.