---
id: TC-473
title: Parallel dispatch preserves the full regression surface — placement, retry, SPMC, and session-row tests stay green
type: exit-criteria
status: passing
validates:
  features:
  - FT-181
  adrs:
  - ADR-080
  - ADR-091
phase: 1
runner: cargo-test
runner-args: -p decision-cli -- ft_17 ft_135
runner-timeout: 600
observes:
- exit-code
- stdout
last-run: 2026-06-12T13:19:58.955295907+00:00
last-run-duration: 2.0s
---

## Purpose

FT-181 regression umbrella: the placement (FT-170), retry (FT-171), SPMC (FT-177), and crate-contract (FT-178) test families all pass against the level-parallel dispatcher — output equivalence with sequential dispatch.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.