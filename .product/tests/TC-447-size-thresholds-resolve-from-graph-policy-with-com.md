---
id: TC-447
title: size thresholds resolve from graph policy with compiled defaults on legacy stores
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_175_thresholds_from_policy
runner-timeout: 300
observes:
- graph
---

## Description

Two halves: (1) an orchestration store carrying a size-policy artifact with a custom body-length threshold makes the gate fire at the custom limit; (2) a legacy store without the policy artifact falls back to the compiled defaults, and the defaults sit below the FT-163 truncation cap (a spec that would be truncated must trip the gate first). Asserts on the **graph** (the policy artifact's quads are the values the gate used, recorded in the planner's stuck diagnostics for case 1; the default values recorded for case 2).
