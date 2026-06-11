---
id: TC-420
title: No feature-slice reach below the binary — dec-ontology, dec-graph, and dec-harness never reference crate features
type: invariant
status: passing
validates:
  features:
  - FT-169
  adrs:
  - ADR-086
phase: 1
runner: bash
runner-args: scripts/checks/no-feature-reach-below-binary.sh
runner-timeout: 30
observes:
- exit-code
- stdout
last-run: 2026-06-11T14:06:27.760040469+00:00
last-run-duration: 0.0s
---

## Purpose

Invariant for [ADR-086](ADR-086) (witnessed by [FT-169](FT-169)): the crates below the binary — `dec-ontology`, `dec-graph`, `dec-harness` — never reference the binary's feature slices. Cargo makes a real import a compile error; this audit additionally catches doc links and commented code that normalise the upward reach. The pre-FT-169 tree contained exactly that rot: a production import in the trace writer (`features::ft_116`), one in cluster session persistence (`features::implement`), a test import (`features::submissions`), and three rustdoc links — each found and removed during the extraction.

## Mechanism

Backed by `scripts/checks/no-feature-reach-below-binary.sh`: greps `crate::features` and `decision_cli::features` across the three lower crates' `src/` trees.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0; stdout reports `OK: no feature-slice reach below the binary crate`.

## Fail criteria

Exit-code 1; stdout lists each offending `file:line`.