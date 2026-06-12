---
id: TC-443
title: cell declaring a tool outside the role surface fails validation before any dispatch
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_174_widening_is_validation_error
runner-timeout: 300
observes:
- exit-code
- stderr
---

## Description

A cell declares `tools: [run_release]` while the role surface contains no such tool. The cluster run must abort during the pre-dispatch validation pass (ADR-088 §3): asserts on **exit-code** (the run fails before any worker subprocess spawns — the stub worker records zero invocations) and on **stderr** (the diagnostic names the cell, the offending tool, and the role surface, so the operator can see both sides of the mismatch).
