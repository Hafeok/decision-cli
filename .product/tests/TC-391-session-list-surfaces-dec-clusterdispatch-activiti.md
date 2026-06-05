---
id: TC-391
title: session list surfaces dec:ClusterDispatch activities with featureId and clusterOutcome
type: exit-criteria
status: passing
validates:
  features:
  - FT-162
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_162_session_list_cluster tc_391_session_list_surfaces_cluster_dispatch_activities
runner-timeout: 120
observes:
- stdout
last-run: 2026-06-05T09:52:39.579981575+00:00
last-run-duration: 0.2s
---

## Description

Exit-criteria test for [FT-162](FT-162) §Outputs branch 3 (cluster dispatch activities surface in list). Persists a single-cell cluster run, then asserts the parent activity IRI appears in `list` output with the correct projected fields.

## Assertions

1. The cluster IRI is present in the list output.
2. The row's `feature_id` field equals the persisted `FT-T391`.
3. The row's `status` field equals `"succeeded"` — the projection of `dec:clusterOutcome` onto the standard `?status` column.

## Runner

`cargo-test` of `tc_391_session_list_surfaces_cluster_dispatch_activities`.