---
id: TC-027
title: every ActionSession has a paired InterpretationSession via DispatchGroup
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
runner-args: scripts/checks/action-interpretation-pairing.sh
runner-timeout: 60
last-run: 2026-05-20T08:40:46.051275612+00:00
last-run-duration: 0.0s
---

## Description

[Describe test here.]