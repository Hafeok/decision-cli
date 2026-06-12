---
id: TC-449
title: cluster run aborts with budget-exceeded session record before the next dispatch once the ceiling is reached
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_176_cluster_budget_exceeded_abort
runner-timeout: 300
observes:
- graph
- disk-state
---

## Description

A fixture cluster run with a tiny declared TaskType budget and a stub worker reporting large usage must abort before dispatching the next cell. Asserts on the **graph** (the cluster SessionRecord carries the structured `budget-exceeded` failure with spent-vs-declared per token class, and no session quads exist for the never-dispatched cells) and on **disk-state** (the sandbox is preserved with the completed cells' outputs intact — the same operator surface as audit-cap exhaustion).
