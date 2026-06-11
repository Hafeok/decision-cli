---
id: TC-436
title: Retried cells receive the audit diagnostic and stale dependents are derivable from the cell graph
type: invariant
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli -- ft_171_bundle_carries_prior_audit_failure ft_171_dependents_of_direct_edges
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T18:25:13.418545770+00:00
last-run-duration: 3.0s
---

## Description

[Describe test here.]