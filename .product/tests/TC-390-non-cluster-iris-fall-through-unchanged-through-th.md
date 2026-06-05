---
id: TC-390
title: non-cluster IRIs fall through unchanged through the existing slice-1 renderer
type: scenario
status: passing
validates:
  features:
  - FT-161
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_161_session_show_cluster tc_390_non_cluster_iri_falls_through_to_existing_renderer
runner-timeout: 120
observes:
- stdout
last-run: 2026-06-05T09:37:33.400636441+00:00
last-run-duration: 0.4s
---

## Description

Scenario test for [FT-161](FT-161) §Invariants "No regression on the existing slice-1 renderer". Routes an IRI that matches neither cluster prefix and asserts the slice-1 path's error verbiage surfaces — not a cluster-flavoured error.

## Assertions

1. `session_show` returns Err for an unknown IRI shape.
2. The error message contains the exact slice-1 prefix `"no Session with IRI"`.
3. The error message does **not** mention `"cluster"` — pins the router's contract that non-cluster IRIs never touch the cluster renderers.

## Runner

`cargo-test` invocation of `tc_390_non_cluster_iri_falls_through_to_existing_renderer`.