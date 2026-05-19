---
id: TC-027
title: every ActionSession has a paired InterpretationSession via DispatchGroup
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
runner-args: scripts/checks/action-interpretation-pairing.sh
runner-timeout: 60
---

## Description

[Describe test here.]
