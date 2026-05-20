---
id: TC-034
title: Feedback class is in the controlled vocabulary
type: invariant
status: failing
validates:
  features:
  - FT-028
  adrs:
  - ADR-023
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test feedback_class_vocab
runner-timeout: 120
last-run: 2026-05-20T08:26:41.315265110+00:00
last-run-duration: 0.1s
failure-message: "error: no test target named `feedback_class_vocab` in `decision-cli` package\nhelp: available test targets:\n    tc_012_session_invariants\n    tc_014_in_stream_invariant\n    tc_015_bootstrap_session\n    tc_018_finalize_commit_and_status\n    tc_019_bootstrap_subscriptions\n"
---

## Description

[Describe test here.]