---
id: TC-184
title: Matcher short-circuit still fires when no defect feedback exists
type: exit-criteria
status: failing
validates:
  features:
  - FT-107
  adrs:
  - ADR-030
phase: 3
runner: cargo-test
runner-args: tc_184_matcher_short_circuit_still_fires
runner-timeout: 60
last-run: 2026-05-28T08:49:13.813112667+00:00
last-run-duration: 0.3s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

When no `dec:Feedback` artifacts exist for the `(feature, env)` pair (or all existing ones are `addressed`/`obsolete`), the matcher's complete-coverage short-circuit fires as before FT-107 — `run_generate` returns a `Match` proposal and never invokes the worker. The slice does not regress the green path.

## Scenarios

### Setup

- A fresh `dec init`-bootstrapped tree.
- One feature_spec (`FT-T2`) with one TC.
- One env (`ENV-T2`).
- One `dec:VerificationGraph` (`VG-T2`) verifying `FT-T2` in `ENV-T2` that completely covers `FT-T2`'s TCs.
- No defect feedback for `(FT-T2, ENV-T2)`.

### Test

Call `verify_graph_generate::run_generate` for `(FT-T2, ENV-T2)`. Assert:

1. The handler returns a proposal with `kind == Match` referencing `VG-T2`.
2. No process spawn for `verify-graph-author` (the worker invoker is the same path as FT-080's test instrumentation hook).

### Boundary

- A `dec:Feedback` with `lifecycleState = "addressed"` for the same `(feature, env)` does NOT block the short-circuit — only `produced` feedback counts.