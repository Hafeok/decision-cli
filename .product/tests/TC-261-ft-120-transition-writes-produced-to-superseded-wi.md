---
id: TC-261
title: FT-120 transition writes produced-to-superseded with supersededByTopologyChange predicate
type: scenario
status: passing
validates:
  features:
  - FT-120
  adrs:
  - ADR-024
phase: 4
runner: cargo-test
runner-args: features::ft_120_retract_orphan_defects::tests::tc_261_produced_to_superseded
runner-timeout: 30
last-run: 2026-06-01T09:36:07.533566345+00:00
last-run-duration: 0.7s
---

## Description

`features::ft_120_retract_orphan_defects::transition::retract_orphan`
applies the legal ADR-024 transition and writes the required
predicates atomically.

## Acceptance criteria

1. **Produced → superseded transition.** Calling `retract_orphan(fb,
   session)` on a feedback in `produced` state transitions it to
   `superseded`. SHACL validation passes.
2. **Routed → superseded transition.** Same as (1) starting from
   `routed` state.
3. **Received state not retracted.** Calling `retract_orphan` on a
   feedback in `received` state returns an error or no-ops (ADR-024
   does not include `received → superseded` in the legal transitions).
4. **Predicate set.** The post-transition triples include
   `<fb> dec:supersededByTopologyChange <session>`,
   `<fb> dec:supersededAt <timestamp>`, and a non-empty
   `dec:supersededReason` literal.
5. **subPropertyOf relationship.** A SPARQL query for
   `?fb dec:supersededBy ?x` returns the same session IRI as
   `?fb dec:supersededByTopologyChange ?x`, confirming the
   subPropertyOf wiring.
6. **Terminal-state rejection.** Calling `retract_orphan` on a
   feedback already in `superseded` / `closed` / `rejected` returns
   an error and writes nothing.

## Runner

`cargo-test` against the new module's `tests.rs`.