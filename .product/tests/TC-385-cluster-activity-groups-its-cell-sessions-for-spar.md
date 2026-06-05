---
id: TC-385
title: Cluster activity groups its cell sessions for SPARQL aggregate rollup
type: scenario
status: passing
validates:
  features:
  - FT-146
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_146_cluster_session_persist ft_146_cluster_activity_groups_its_cell_sessions
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T09:18:18.466844397+00:00
last-run-duration: 0.3s
---

## Description

Scenario test for [FT-146](FT-146) §Outputs: "`dec session show urn:dec:cluster-dispatch:<task-type>/<feature>` renders the aggregate via `aggregate_chain_cost`-style rollup adapted for siblings rather than chains."

Persists a cluster run with three cells (base tokens 100/200/300) and asserts SPARQL can walk `prov:wasInformedBy` to enumerate the cluster's children and aggregate token usage with `SUM(xsd:integer(?base))`. This is the queryable shape that powers the cost rollup the spec promises operators.

## Assertions

1. SPARQL `SELECT ?cell WHERE { ?cell prov:wasInformedBy <cluster> }` returns exactly 3 rows.
2. SPARQL `SELECT (SUM(xsd:integer(?base)) AS ?total)` over the cluster's children returns `600` (100 + 200 + 300).
3. The aggregate works without needing to know cell IRIs in advance — only the parent cluster IRI.

## Runner

`cargo-test` invocation of `ft_146_cluster_activity_groups_its_cell_sessions`.