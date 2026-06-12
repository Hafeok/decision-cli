---
id: TC-459
title: Crate-contract bundle naming is explicit — oxrdf required, oxigraph forbidden in the instruction
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
last-run-duration: 2.0s
---

## Purpose

FT-178: the contract text is explicit about the dependency universe — requires `oxrdf`, forbids `oxigraph` by name (the witnessed FT-148 compile-failure class). Runner shares the bundle test, which asserts both literals.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.