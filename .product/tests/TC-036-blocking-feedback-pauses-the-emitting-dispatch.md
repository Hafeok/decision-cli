---
id: TC-036
title: blocking feedback pauses the emitting dispatch
type: exit-criteria
status: failing
validates:
  features:
  - FT-032
  adrs:
  - ADR-025
phase: 2
runner: bash
runner-args: scripts/checks/feedback-blocking-pauses.sh
runner-timeout: 120
---

## Description

[Describe test here.]
