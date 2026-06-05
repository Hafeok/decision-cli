---
id: TC-375
title: dec product status returns summary text and exits 0 against a populated graph
type: scenario
status: failing
validates:
  features:
  - FT-145
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-375-dec-product-status-text.sh
runner-timeout: 30
observes:
- stdout
- exit-code
last-run: 2026-06-05T06:05:12.556078839+00:00
last-run-duration: 0.0s
failure-message: ""
---

## Description

[Describe test here.]