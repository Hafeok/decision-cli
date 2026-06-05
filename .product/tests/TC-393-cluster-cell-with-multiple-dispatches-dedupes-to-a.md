---
id: TC-393
title: cluster cell with multiple dispatches dedupes to a single row via GROUP BY
type: scenario
status: passing
validates:
  features:
  - FT-162
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_162_session_list_cluster tc_393_cluster_cell_with_multiple_dispatches_dedupes_to_single_row
runner-timeout: 120
observes:
- stdout
last-run: 2026-06-05T09:52:39.579981575+00:00
last-run-duration: 0.3s
---

## Description

Scenario test for [FT-162](FT-162) §Behaviour `GROUP BY ?session` dedupe. Persists the same cluster IRI twice with the same cell (different statuses + different outcomes per run), then asserts the list output has exactly one row per IRI.

## Assertions

1. The handler cell IRI appears exactly **once** in the list output across the two dispatches.
2. The cluster dispatch IRI appears exactly **once** in the list output.

Pins FT-162 §Invariants stable ordering + GROUP BY dedupe contract.

## Runner

`cargo-test` of `tc_393_cluster_cell_with_multiple_dispatches_dedupes_to_single_row`.