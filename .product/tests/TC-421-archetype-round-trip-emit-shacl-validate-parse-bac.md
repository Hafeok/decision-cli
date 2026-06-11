---
id: TC-421
title: Archetype round-trip — emit, SHACL-validate, parse back, structural equality with three seam audits
type: exit-criteria
status: passing
validates:
  features:
  - FT-147
  adrs:
  - ADR-082
  - ADR-084
  - ADR-085
phase: 1
runner: cargo-test
runner-args: -p dec-ontology round_trip_with_three_seam_audits
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T17:46:12.116126695+00:00
last-run-duration: 0.1s
---

## Purpose

FT-147 spec test case 1: build an [`Archetype`] with three seam audits, emit quads, validate against the ADR-082 shape, parse back, and assert structural equality — the parser+emitter field-coverage symmetry the `add-artifact-type` coherence audit demands, proven at runtime.

## Mechanism

`cargo test -p dec-ontology round_trip_with_three_seam_audits` — runs `crates/dec-ontology/src/ontology/archetype/tests.rs::round_trip_with_three_seam_audits` against the cluster-authored (operator-promoted) archetype module.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — emit → validate → parse round-trips byte-equal including the dual-provenance block (FT-072/073).

## Fail criteria

Exit-code non-zero; stdout names the asymmetric field.