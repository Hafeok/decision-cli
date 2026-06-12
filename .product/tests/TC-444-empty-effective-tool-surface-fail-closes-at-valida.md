---
id: TC-444
title: empty effective tool surface fail-closes at validation with structured session failure
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_174_empty_intersection_fail_closed
runner-timeout: 300
observes:
- exit-code
- graph
---

## Description

A legacy store without `dec:roleTool` quads (or a narrowing whose intersection with the role surface is empty) must fail-close at validation. Asserts on **exit-code** (the cluster run aborts before any dispatch; no tokens spent) and on the **graph** (the persisted session record carries the structured validation failure naming the empty surface, mirroring ADR-070's `invalid_dispatch` fail-closed semantics).
