---
id: TC-410
title: resolve_cell_output_path rejects dotdot traversal as sandbox containment guard
type: scenario
status: passing
validates:
  features:
  - FT-166
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_166_resolve_cell_output_path_rejects_dotdot_traversal
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T12:27:44.473444561+00:00
last-run-duration: 0.2s
---

## Description

Scenario test for [FT-166](FT-166) §Invariants sandbox-containment guard. A resolved cell `output_path` containing a `..` segment after substitution must reject. Pins the security property that a misconfigured (or hostile) parameter cannot escape the cluster sandbox.

## Assertions

1. A parameter value containing `../../etc/passwd` substituted into `crates/{artifact_name}.rs` produces a path with `..` segments.
2. `resolve_cell_output_path` returns `Err` with `sandbox containment guard` in the diagnostic.

Pins FT-166 §Invariants structural-containment property.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_166_resolve_cell_output_path_rejects_dotdot_traversal`.