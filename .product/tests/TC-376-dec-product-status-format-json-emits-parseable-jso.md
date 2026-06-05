---
id: TC-376
title: dec product status --format json emits parseable JSON containing per-phase counts
type: scenario
status: failing
validates:
  features:
  - FT-145
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-376-dec-product-status-json.sh
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