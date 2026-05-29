---
id: TC-200
title: Sweep resolves features in numeric-suffix ascending order
type: scenario
status: passing
validates:
  features:
  - FT-111
  adrs: []
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_200_sweep_resolves_features_in_numeric_suffix_order
runner-timeout: 30
last-run: 2026-05-29T09:26:03.704087369+00:00
last-run-duration: 0.8s
---

## Description

Resolver returns the feature list sorted by numeric suffix
ascending so two sweep invocations against the same store
produce byte-identical row ordering. Without this, the bash
script ancestor's "sort -u" lottery returned a different order on
every run depending on the shell's locale.

## Acceptance Criteria

Given a product graph with features `FT-3`, `FT-10`, `FT-2`,
`FT-100`, the resolver returns `["FT-2", "FT-3", "FT-10",
"FT-100"]` (lexicographic on the full ID would give
`FT-10, FT-100, FT-2, FT-3` — that ordering is the failure
mode this test pins).

The test seeds an in-memory product graph fixture, calls the
resolver, and asserts on the Vec.