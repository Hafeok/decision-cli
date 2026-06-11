---
id: TC-419
title: dec-harness test suite — relocated dispatch, drive, worker, and subscription tests pass in the extracted crate
type: exit-criteria
status: passing
validates:
  features:
  - FT-169
  adrs:
  - ADR-086
phase: 1
runner: bash
runner-args: scripts/checks/tc-419-dec-harness-tests.sh
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T14:06:27.760040469+00:00
last-run-duration: 5.8s
---

## Purpose

Exit criterion for [FT-169](FT-169) ([ADR-086](ADR-086)): the dispatch, drive-planner, worker-contract, subscription, and verification-orchestration tests that moved with the harness pass **unmodified** in their new home.

## Mechanism

Backed by `scripts/checks/tc-419-dec-harness-tests.sh`, which runs `cargo test -p dec-harness --quiet` and propagates its exit-code.

(Two corrections rode the move: the artifact-id parser gained the `BNCH-` arm that [FT-112](FT-112)'s rename promised — `happy_path_prefixes` and integration test `tc_195` had been failing since the rename — and the promoted `verify::stale_defects` machinery is exercised here rather than through the feature slice.)

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — every test in the extracted crate passes.

## Fail criteria

Exit-code 1 — the crate is missing or a relocated test fails; stdout carries the cargo test report.