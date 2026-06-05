---
id: TC-388
title: session show renders a cluster-dispatch IRI with per-cell table and currency total
type: scenario
status: passing
validates:
  features:
  - FT-161
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_161_session_show_cluster tc_388_cluster_dispatch_iri_renders_table_with_currency_total
runner-timeout: 120
observes:
- stdout
last-run: 2026-06-05T09:37:33.400636441+00:00
last-run-duration: 0.4s
---

## Description

Scenario test for [FT-161](FT-161) §Outputs cluster-dispatch renderer. Seeds a capability with EUR cost rates (0.20/M input, 0.80/M output), persists a 3-cell cluster run (2 priced LLM-cells + 1 mechanical), then renders.

## Assertions

1. Header lines render `Feature`, `Task type`, `Outcome`.
2. `Cells (3):` header (cell count).
3. Each of the three cell short-names appears in the table body.
4. The Euro currency symbol `€` appears (priced cells).
5. A `TOTAL EUR` row is emitted (currency-tagged total).
6. Aggregate base tokens (`4000`) and aggregate output (`800`) appear in the TOTAL row.

## Runner

`cargo-test` invocation of `tc_388_cluster_dispatch_iri_renders_table_with_currency_total`.