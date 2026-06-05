---
id: TC-397
title: long feature_spec truncates with the cluster-framing witness suffix
type: scenario
status: passing
validates:
  features:
  - FT-163
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_163_long_spec_truncates_with_witness_suffix
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T10:33:54.266178533+00:00
last-run-duration: 0.2s
---

## Description

Scenario test for [FT-163](FT-163) §Behaviour — specs exceeding the cap truncate with the witness suffix appended. Pins both the char-counting (not byte-counting) prefix length and the suffix's documenting role.

## Assertions

For a 60k-char input against the 50k cap:
1. The output contains `[spec truncated for cell framing]` — the witness suffix.
2. The first `MAX_FRAMING_CHARS` chars of the output total exactly `MAX_FRAMING_CHARS` chars (truncation is char-based, not byte-based).

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_163_long_spec_truncates_with_witness_suffix`.