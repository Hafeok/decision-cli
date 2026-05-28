---
id: TC-134
title: WorkerCurator session admits a clean Submission and rejects a flawed one
type: exit-criteria
status: failing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_134_workercurator_session_admits_a_clean_submission_an
runner-timeout: 120
last-run: 2026-05-28T08:48:40.372514383+00:00
last-run-duration: 0.3s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Description

[Describe the test criterion here.]