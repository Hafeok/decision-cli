---
id: TC-435
title: Audit-repair degrades unmapped failures to every cell — never silently narrower
type: invariant
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_171_unmapped_failure_implicates_all_cells
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T18:25:13.418545770+00:00
last-run-duration: 0.9s
---

## Description

[Describe test here.]