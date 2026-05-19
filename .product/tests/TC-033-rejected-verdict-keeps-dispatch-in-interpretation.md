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
---

## Description

[Describe test here.]
