---
id: TC-389
title: cluster-dispatch renderer dedupes multi-run cells and annotates aggregated outcomes
type: scenario
status: passing
validates:
  features:
  - FT-161
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_161_session_show_cluster tc_389_cluster_dispatch_dedupes_multi_run_and_annotates_outcomes
runner-timeout: 120
observes:
- stdout
last-run: 2026-06-05T09:37:33.400636441+00:00
last-run-duration: 0.5s
---

## Description

Scenario test for [FT-161](FT-161) §Behaviour multi-iteration aggregation. Persists the same `urn:dec:cluster-dispatch:*` IRI **twice** with the same cell (different statuses + different outcomes), then renders.

## Assertions

1. The cell's short name appears exactly **once** in the output — the per-cell dedupe via SPARQL `GROUP BY ?cell` + `SAMPLE` works correctly.
2. The header carries the `runs aggregated` annotation indicating multiple `dec:clusterOutcome` values were observed.

This pins FT-161 §Invariants stable cell ordering + multi-run aggregation.

## Runner

`cargo-test` invocation of `tc_389_cluster_dispatch_dedupes_multi_run_and_annotates_outcomes`.