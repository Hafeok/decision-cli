---
id: TC-431
title: Audit compile probe rejects non-compiling emissions with the rustc diagnostic
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
runner-args: scripts/checks/tc-431-audit-compile-probe-negative.sh
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T18:18:31.674452582+00:00
last-run-duration: 0.7s
---

## Purpose

FT-172: the compile probe rejects emitted Rust that does not type-check against HEAD — the second blind spot witnessed on FT-147 (const `NamedNode`, `Eq` over `f32`, phantom types all passed the structural audit). The FAIL line carries the rustc diagnostic so FT-171 can feed it back to the offending cell.

## Mechanism

`scripts/checks/tc-431-audit-compile-probe-negative.sh` injects a syntax error into a fixture copy of the vocab cell and asserts exit-code 1 with `check=compile_probe`.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — broken Rust was refused with the compile_probe check named.

## Fail criteria

Exit-code 1 — non-compiling Rust passed the audit or failed on a different check.