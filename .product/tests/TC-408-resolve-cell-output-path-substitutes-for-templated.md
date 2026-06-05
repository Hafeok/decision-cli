---
id: TC-408
title: resolve_cell_output_path substitutes for templated paths and falls back for empty
type: scenario
status: passing
validates:
  features:
  - FT-166
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_166_resolve_cell_output_path_substitutes_or_falls_back
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T12:27:44.473444561+00:00
last-run-duration: 0.3s
---

## Description

Scenario test for [FT-166](FT-166) §Behaviour cell-path resolution. Covers both the templated and the flat-path-fallback branches.

## Assertions

1. A cell with `output_path = "crates/decision-cli/src/core/ontology/{artifact_name}.rs"` resolves to `crates/decision-cli/src/core/ontology/feedback.rs` when `artifact_name=feedback`.
2. A cell with empty `output_path` falls back to the FT-139 flat-path convention via `cell_filename` — pins backwards-compatibility for FT-145's add-cli-subcommand cluster.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_166_resolve_cell_output_path_substitutes_or_falls_back`.