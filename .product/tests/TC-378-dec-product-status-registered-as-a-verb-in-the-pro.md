---
id: TC-378
title: dec product status registered as a verb in the product_cmd dispatcher table
type: scenario
status: failing
validates:
  features:
  - FT-145
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-378-dec-product-status-registered.sh
runner-timeout: 30
observes:
- exit-code
last-run: 2026-06-05T06:05:12.556078839+00:00
last-run-duration: 0.0s
failure-message: "TC-378 FAIL: status not registered — dispatcher returned 'unknown subcommand'\n"
---

## Description

[Describe test here.]