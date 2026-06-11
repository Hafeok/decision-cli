---
id: TC-417
title: dec-graph test suite — relocated store, chokepoint, and SHACL tests pass unmodified in the extracted crate
type: exit-criteria
status: passing
validates:
  features:
  - FT-168
  adrs:
  - ADR-086
phase: 1
runner: bash
runner-args: scripts/checks/tc-417-dec-graph-tests.sh
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T13:46:50.631871209+00:00
last-run-duration: 0.7s
---

## Purpose

Exit criterion for [FT-168](FT-168) ([ADR-086](ADR-086)): the store, GraphWriter-chokepoint, and store-aware SHACL tests that moved with the graph-access layer pass **unmodified** in their new home — proof the move changed where the code lives, not what it does.

## Mechanism

Backed by `scripts/checks/tc-417-dec-graph-tests.sh`, which runs `cargo test -p dec-graph --quiet` and propagates its exit-code.

(One assertion was corrected during the move: `missing_bench_type_fails_shacl` still expected the pre-FT-112 `envType` token in the violation report and had been failing since the ENV→BNCH rename; the assertion now matches the `benchType` message the validator actually emits.)

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — every test in the extracted crate passes.

## Fail criteria

Exit-code 1 — the crate is missing or at least one relocated test fails; stdout carries the cargo test report.