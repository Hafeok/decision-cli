---
id: TC-386
title: Cluster session quads carry token breakdown, parent link, and usage source
type: scenario
status: passing
validates:
  features:
  - FT-146
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib core::graph::cluster_session::tests
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T09:18:18.466844397+00:00
last-run-duration: 7.4s
---

## Description

Unit-level scenario tests in `core::graph::cluster_session::tests` covering the quad-emission shape of [FT-146](FT-146)'s helpers — the layer below SHACL / persistence. Pins the wire format the integration tests (TC-383..TC-385) depend on.

## Assertions

Three unit tests in the module:

1. **`cell_quads_emit_token_breakdown_and_links_for_scaleway`** — for a Scaleway cell with non-zero base + output and zero cache fields: `rdf:type dec:Session` present, `dec:capability` link present, all four token predicates with the expected values, `prov:wasInformedBy` link to the parent cluster, `dec:usageSource = "worker-reported"`.
2. **`mechanical_cell_records_zero_tokens_and_unreported_source`** — for a mechanical cell with `usage: None`: `dec:cellStatus = "mechanical"`, `dec:usageSource = "unreported"`, all four token predicates emit literal `"0"`.
3. **`cluster_activity_quads_carry_type_timing_and_outcome`** — for the parent cluster activity: rdf:type carries both `prov:Activity` and `dec:ClusterDispatch`; `dec:clusterOutcome = "succeeded"`; both `prov:startedAtTime` and `prov:endedAtTime` present.

These tests have no store dependency — they test pure quad construction. Pair with the integration tests for full coverage (in-memory shape + persistence + SPARQL round-trip).

## Runner

`cargo-test` invocation of `core::graph::cluster_session::tests` via the library test target.