---
id: TC-033
title: rejected verdict keeps dispatch in interpretation-rejected status
type: exit-criteria
status: passing
validates:
  features:
  - FT-021
  adrs:
  - ADR-017
phase: 2
runner: bash
runner-args: scripts/checks/dispatch-rejected-stays-blocked.sh
runner-timeout: 120
last-run: 2026-05-23T18:00:16.213042199+00:00
last-run-duration: 0.1s
---

## Description

[Describe test here.]