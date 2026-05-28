---
id: TC-150
title: 'pipeline-worker SDK: Build-time codegen of typed Bundle and Artifact surfaces from SHACL — exit criterion'
type: exit-criteria
status: failing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_150_pipeline_worker_sdk_build_time_codegen_of_typed_bu
runner-timeout: 120
last-run: 2026-05-28T08:48:38.386243763+00:00
last-run-duration: 0.3s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Description

[Describe the test criterion here.]