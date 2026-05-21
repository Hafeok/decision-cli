---
id: TC-081
title: dec verify graph generate rolls back graph when any step-add fails
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: tc_081_dec_verify_graph_generate_rolls_back_graph_when_an
runner-timeout: 120
last-run: 2026-05-21T19:20:28.691484988+00:00
last-run-duration: 0.5s
---

## Premise

`dec verify graph generate FT-N --environment ENV-1 --accept` is invoked. The worker returns a `New` proposal with two steps. The first step-add succeeds; the second step-add fails SHACL validation (its `--field` map is missing a required key — simulated by injecting a malformed `ProposedStep`).

## Acceptance Criteria

- The graph file at `.dec/verify/graph/VG-NNN.ttl` does **not** exist after the failure (rolled back).
- The store projection no longer contains the partial graph (the named-graph state is consistent with on-disk).
- The handler returns `Error::SchemaViolation` with the failing step's diagnostic.
- Exit code is 1.
- No partial graph is observable by `dec verify graph list` or `dec verify graph show`.

## Notes

Atomic-per-graph acceptance is a contract requirement: a half-written graph would corrupt the coverage queries and confuse the chain gate. The rollback covers both filesystem and store projection.