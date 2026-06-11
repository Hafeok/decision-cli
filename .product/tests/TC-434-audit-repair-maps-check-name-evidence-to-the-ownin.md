---
id: TC-434
title: Audit-repair maps check-name evidence to the owning cell without degrading to all cells
type: invariant
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_171_implicate_by_check_name
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T18:25:13.418545770+00:00
last-run-duration: 2.3s
---

## Description

[Describe test here.]