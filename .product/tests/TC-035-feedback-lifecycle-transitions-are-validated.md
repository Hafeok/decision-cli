---
id: TC-035
title: Feedback lifecycle transitions are validated
type: invariant
status: failing
validates:
  features:
  - FT-027
  adrs:
  - ADR-024
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test feedback_lifecycle
runner-timeout: 180
last-run: 2026-05-20T11:41:36.841111001+00:00
last-run-duration: 0.2s
failure-message: "error: no test target named `feedback_lifecycle` in `decision-cli` package\nhelp: available test targets:\n    ft_019_verifier_role_catalog\n    ft_022_verifier_dispatch_subscription\n    ft_024_agreement_metric\n    ft_025_session_show_paired\n    tc_012_session_invariants\n    tc_014_in_stream_invariant\n    tc_015_bootstrap_session\n    tc_018_finalize_commit_and_status\n    tc_019_bootstrap_subscriptions\n    verdict_rejected_cites\n    verdict_shacl\n"
---

## Description

[Describe test here.]