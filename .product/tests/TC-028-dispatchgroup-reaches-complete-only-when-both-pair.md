---
id: TC-028
title: DispatchGroup reaches complete only when both paired sessions are terminal
type: invariant
status: failing
validates:
  features:
  - FT-021
  - FT-022
  adrs:
  - ADR-017
phase: 2
runner: bash
runner-args: scripts/checks/dispatch-complete-paired-terminal.sh
runner-timeout: 60
last-run: 2026-05-20T08:26:41.315265110+00:00
last-run-duration: 0.0s
failure-message: "bash: line 1: scripts/checks/dispatch-complete-paired-terminal.sh: No such file or directory\n"
---

## Description

[Describe test here.]