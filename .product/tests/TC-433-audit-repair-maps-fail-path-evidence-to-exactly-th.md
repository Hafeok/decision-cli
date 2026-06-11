---
id: TC-433
title: Audit-repair maps FAIL path evidence to exactly the offending cell
type: invariant
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_171_implicate_by_path_evidence
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T18:25:13.418545770+00:00
last-run-duration: 153.1s
---

## Description

[Describe test here.]