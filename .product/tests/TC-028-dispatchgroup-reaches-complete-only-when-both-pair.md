---
id: TC-028
title: DispatchGroup reaches complete only when both paired sessions are terminal
type: invariant
status: passing
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
last-run: 2026-05-20T11:41:36.841111001+00:00
last-run-duration: 0.1s
---

## Description

[Describe test here.]