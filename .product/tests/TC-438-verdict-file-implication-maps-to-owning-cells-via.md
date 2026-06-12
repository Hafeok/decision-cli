---
id: TC-438
title: verdict file implication maps to owning cells via output_path
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_173_verdict_file_implication
runner-timeout: 300
observes:
- graph
- disk-state
---

## Description

A stub `v1` audit fails with a check whose `implicates` carries only `files` (sandbox-relative paths), no cell names. The harness must join those paths against the cells' declared `output_path`s (after parameter substitution) and re-dispatch the owning cells. Asserts on **disk-state** (only the owning cells' outputs are rewritten in the sandbox) and on the **graph** (the SessionRecord attributes the repair to the file→cell mapping, not to the all-cells fallback).
