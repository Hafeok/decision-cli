---
id: TC-416
title: dec-ontology test suite — relocated unit and round-trip tests pass unmodified in the extracted crate
type: exit-criteria
status: passing
validates:
  features:
  - FT-167
  adrs:
  - ADR-086
phase: 1
runner: bash
runner-args: scripts/checks/tc-416-dec-ontology-tests.sh
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T13:27:53.033915894+00:00
last-run-duration: 1.9s
---

## Purpose

Exit criterion for [FT-167](FT-167) ([ADR-086](ADR-086)): the invariant the migration must preserve is that the relocated unit and round-trip tests pass **unmodified** in their new home — proof the move changed where code lives, not what it does.

## Mechanism

Backed by `scripts/checks/tc-416-dec-ontology-tests.sh`, which runs `cargo test -p dec-ontology --quiet` (the relocated SHACL-validator, quad round-trip, and vocab tests) and propagates its exit-code.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — every test in the extracted crate passes.

## Fail criteria

Exit-code 1 — the crate is missing or at least one relocated test fails; stdout carries the cargo test report.