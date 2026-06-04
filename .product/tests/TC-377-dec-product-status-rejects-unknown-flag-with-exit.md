---
id: TC-377
title: dec product status rejects unknown flag with exit 2 and a clear stderr message
type: scenario
status: passing
validates:
  features:
  - FT-145
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-377-dec-product-status-unknown-flag.sh
runner-timeout: 30
observes:
- exit-code
- stderr
last-run: 2026-06-04T19:12:26.965262357+00:00
last-run-duration: 0.0s
---

## Description

[Describe test here.]