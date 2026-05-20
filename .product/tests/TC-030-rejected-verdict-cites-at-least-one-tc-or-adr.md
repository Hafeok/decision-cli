---
id: TC-030
title: rejected verdict cites at least one TC or ADR
type: invariant
status: failing
validates:
  features:
  - FT-020
  - FT-023
  adrs:
  - ADR-018
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test verdict_rejected_cites
runner-timeout: 120
last-run: 2026-05-20T08:26:41.315265110+00:00
last-run-duration: 0.1s
failure-message: "error: no test target named `verdict_rejected_cites` in `decision-cli` package\nhelp: available test targets:\n    tc_012_session_invariants\n    tc_014_in_stream_invariant\n    tc_015_bootstrap_session\n    tc_018_finalize_commit_and_status\n    tc_019_bootstrap_subscriptions\n"
---

## Description

[Describe test here.]