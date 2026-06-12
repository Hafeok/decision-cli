---
id: TC-446
title: feature under all size thresholds dispatches unchanged
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_175_under_threshold_dispatches
runner-timeout: 300
observes:
- graph
---

## Description

Regression guard: a DoR-clean feature under every size threshold must flow through the drive planner exactly as before the size gate landed. Asserts on the **graph** (the dispatch session quads for the feature exist and the planner's recorded round history contains no size-gate entry — the gate evaluated and passed silently).
