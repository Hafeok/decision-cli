---
id: TC-457
title: add-artifact-type cell bundles carry the crate contract and distilled existing interfaces
type: invariant
status: passing
validates:
  features: [FT-178]
  adrs: [ADR-091]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_178_bundle_carries_crate_contract_and_interfaces
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T10:51:13.853263711+00:00
last-run-duration: 422.3s
---

## Purpose

FT-178 / ADR-091: every add-artifact-type LLM cell bundle carries the `## Crate contract` block and the `## Existing crate interfaces` section with the distilled surface. Runner: `cargo test -p decision-cli ft_178_bundle_carries_crate_contract_and_interfaces`.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.