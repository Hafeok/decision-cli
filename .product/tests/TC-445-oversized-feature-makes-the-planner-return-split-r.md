---
id: TC-445
title: oversized feature makes the planner return split-required with measured signals and no dispatch
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_175_split_required_gate
runner-timeout: 300
observes:
- graph
- stdout
---

## Description

A fixture product graph contains a feature whose spec body exceeds the body-length threshold. `dec drive` with the ship goal must terminate the round with the `split-required` stuck variant instead of dispatching. Asserts on **stdout** (the drive output names `split-required` and reports the measured size signals against their thresholds) and on the **graph** (no Dispatch/Session quads were created for the feature — zero dispatch occurred).
