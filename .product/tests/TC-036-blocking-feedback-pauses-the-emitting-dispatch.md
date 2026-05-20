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
last-run: 2026-05-20T08:26:41.315265110+00:00
last-run-duration: 0.0s
failure-message: "bash: line 1: scripts/checks/feedback-blocking-pauses.sh: No such file or directory\n"
---

## Description

[Describe test here.]