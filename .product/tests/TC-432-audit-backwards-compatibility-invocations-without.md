---
id: TC-432
title: Audit backwards compatibility — invocations without the cell list audit the whole fixture
type: invariant
status: passing
validates:
  features:
  - FT-172
  adrs:
  - ADR-080
  - ADR-013
phase: 1
runner: bash
runner-args: scripts/checks/tc-432-audit-no-cell-args-fallback.sh
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T18:18:31.674452582+00:00
last-run-duration: 1.6s
---

## Purpose

FT-172 backwards compatibility: the audit's CLI contract grew optional cell-path arguments (passed by the FT-170/172 harness); invocations without them — older harness builds, manual operator runs — degrade to auditing every `.rs`/`.ttl` in the fixture rather than erroring.

## Mechanism

`scripts/checks/tc-432-audit-no-cell-args-fallback.sh` runs the audit with only the fixture argument and asserts the five-check pass.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code 1 — the no-args invocation regressed.