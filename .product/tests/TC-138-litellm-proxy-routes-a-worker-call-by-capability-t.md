---
id: TC-138
title: LiteLLM proxy routes a worker call by capability tag and reports telemetry to pipeline-cli
type: exit-criteria
status: failing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_138_litellm_proxy_routes_a_worker_call_by_capability_t
runner-timeout: 120
last-run: 2026-05-28T08:48:41.629212152+00:00
last-run-duration: 0.4s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Description

[Describe the test criterion here.]