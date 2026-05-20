---
id: TC-038
title: closed feedback references its addressing artifact via PROV-O
type: invariant
status: failing
validates:
  features:
  - FT-027
  adrs:
  - ADR-024
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test feedback_closed_provo
runner-timeout: 120
last-run: 2026-05-20T08:39:57.378014998+00:00
last-run-duration: 0.2s
failure-message: "error: no test target named `feedback_closed_provo` in `decision-cli` package\nhelp: available test targets:\n    ft_019_verifier_role_catalog\n    tc_012_session_invariants\n    tc_014_in_stream_invariant\n    tc_015_bootstrap_session\n    tc_018_finalize_commit_and_status\n    tc_019_bootstrap_subscriptions\n"
---

## Description

[Describe test here.]