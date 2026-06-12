---
id: TC-474
title: ApplicationContract round-trip — six conventions plus cross-cutting entries emit, validate, and parse back equal
type: exit-criteria
status: passing
validates:
  features: [FT-148]
  adrs: [ADR-082]
phase: 1
runner: cargo-test
runner-args: -p dec-ontology round_trip_full_contract
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T13:34:22.523452111+00:00
last-run-duration: 1.5s
---

## Purpose

FT-148 spec test 1: a contract with all six required conventions plus three cross-cutting entries emits, passes SHACL, and parses back structurally equal (inline Convention sub-resources included, dual provenance per FT-072/073).

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.