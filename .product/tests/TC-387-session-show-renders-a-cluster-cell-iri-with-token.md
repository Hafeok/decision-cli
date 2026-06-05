---
id: TC-387
title: session show renders a cluster cell IRI with token breakdown and parent link
type: exit-criteria
status: passing
validates:
  features:
  - FT-161
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_161_session_show_cluster tc_387_cluster_cell_iri_renders_breakdown_and_parent_link
runner-timeout: 120
observes:
- stdout
last-run: 2026-06-05T09:37:33.400636441+00:00
last-run-duration: 0.3s
---

## Description

Exit-criteria test for [FT-161](FT-161) §Outputs cluster-cell renderer. Bootstraps a workdir, persists a single-cell cluster run via [FT-146](FT-146)'s `persist_cluster_run`, then invokes `session_show` on the cell IRI.

## Assertions

The rendered output contains:
1. `Cell session` header carrying the input IRI.
2. The parent cluster IRI on a `Cluster` line.
3. The capability IRI on a `Capability` line.
4. `succeeded` status + `worker-reported` usage source.
5. The base + output token values (`4321` and `765`) right-aligned in the token-breakdown block.

## Runner

`cargo-test` invocation of `tc_387_cluster_cell_iri_renders_breakdown_and_parent_link`.