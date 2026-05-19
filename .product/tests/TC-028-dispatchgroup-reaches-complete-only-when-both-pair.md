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
---

## Description

[Describe test here.]
