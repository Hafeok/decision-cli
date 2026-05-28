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
---

## Description

[Describe test here.]
