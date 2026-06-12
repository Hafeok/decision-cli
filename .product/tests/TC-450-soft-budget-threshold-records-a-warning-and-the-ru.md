---
id: TC-450
title: soft budget threshold records a warning and the run continues
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_176_soft_threshold_warns
runner-timeout: 300
observes:
- graph
---

## Description

A cluster run whose stub-reported usage crosses the soft threshold (default 50% of the ceiling) but stays under the ceiling must run to completion. Asserts on the **graph** (a budget-warning record exists on the SessionRecord naming the threshold and the spend at crossing; the run's terminal status is unaffected by the warning).
