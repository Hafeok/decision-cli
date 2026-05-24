---
id: TC-079
title: dec verify graph generate persists proposed graph through slice-2.5 writers
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: tc_079_dec_verify_graph_generate_persists_proposed_graph
runner-timeout: 120
last-run: 2026-05-24T19:14:10.271430129+00:00
last-run-duration: 0.4s
---

## Premise

`dec verify graph generate FT-P --environment ENV-1 --accept` is invoked against a store where no graph covers `FT-P` in `ENV-1`. The verify-graph-author worker is mocked to return a `New` proposal with three steps covering all of `FT-P`'s TCs.

## Acceptance Criteria

- A new `VG-NNN.ttl` exists at `.dec/verify/graph/` with the proposed env and three steps.
- Each step in the on-disk Turtle has the `dec:providesEvidenceFor` predicate set to the proposed TC ids.
- The graph is registered in the orchestration store (SHACL passed, store projection updated).
- The writes occurred through the slice-2.5 writers — specifically, [FT-041](FT-041)'s `graph new` handler was called once and [FT-044](FT-044)'s `step add` handler was called three times. (Verified via a writer-instrumentation hook or trace assertion.)
- The handler returns `{ graph_id, path, coverage_report }`; `coverage_report.uncovered = []`.
- Exit code is 0.

## Notes

Validates that the entire pipeline (matcher → worker → writers) integrates correctly and that persistence reuses the existing chokepoint without introducing a parallel write path.