---
id: TC-033
title: rejected verdict keeps dispatch in interpretation-rejected status
type: exit-criteria
status: failing
validates:
  features:
  - FT-021
  adrs:
  - ADR-017
phase: 2
runner: bash
runner-args: scripts/checks/dispatch-rejected-stays-blocked.sh
runner-timeout: 120
last-run: 2026-05-20T08:26:41.315265110+00:00
last-run-duration: 0.0s
failure-message: "bash: line 1: scripts/checks/dispatch-rejected-stays-blocked.sh: No such file or directory\n"
---

## Description

[Describe test here.]