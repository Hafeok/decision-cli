---
id: TC-394
title: every IRI returned by session list resolves via session show
type: scenario
status: passing
validates:
  features:
  - FT-162
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_162_session_list_cluster tc_394_every_listed_iri_resolves_via_show
runner-timeout: 120
observes:
- stdout
- exit-code
last-run: 2026-06-05T09:52:39.579981575+00:00
last-run-duration: 0.3s
---

## Description

Scenario test for [FT-162](FT-162) §Invariants list/show totality. Persists a 3-cell cluster, walks every row returned by `list`, and asserts each IRI resolves cleanly via `session_show` — the [ADR-081](ADR-081) totality invariant.

## Assertions

1. `list` returns at least 4 rows (3 cells + 1 cluster dispatch).
2. **Every** IRI returned by `list` resolves via `session_show` without error.
3. The test fails loudly with a list of the unresolved IRIs + their show error messages — pins the structural contract complement FT-162 closes.

## Runner

`cargo-test` of `tc_394_every_listed_iri_resolves_via_show`.