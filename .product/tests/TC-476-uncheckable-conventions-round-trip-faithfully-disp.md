---
id: TC-476
title: Uncheckable conventions round-trip faithfully — dispatchability consequence deferred to FT-150/153
type: invariant
status: passing
validates:
  features: [FT-148]
  adrs: [ADR-082]
phase: 1
runner: cargo-test
runner-args: -p dec-ontology uncheckable_convention_round_trips
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T13:34:22.523452111+00:00
last-run-duration: 0.1s
---

## Purpose

FT-148 spec test 4 (placeholder per spec): `checkable: false` is valid and round-trips faithfully; the not-safely-dispatchable propagation ships in FT-150/FT-153 and is asserted there.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.