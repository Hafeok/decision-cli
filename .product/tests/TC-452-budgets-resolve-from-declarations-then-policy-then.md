---
id: TC-452
title: budgets resolve from declarations then policy then compiled defaults with explicit unlimited opt-out
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_176_budget_resolution_order
runner-timeout: 300
observes:
- graph
---

## Description

Budget resolution order per ADR-090 §2, asserted on the **graph** (the resolved budget recorded on each run's session record names its source):

1. A TaskType/RoleBinding declaring a budget wins over the policy artifact.
2. Absent a declaration, the orchestration-store policy artifact's default applies.
3. On a legacy store with neither, the compiled default applies — never infinity.
4. An explicit `unlimited` declaration disables enforcement and is recorded as such.
