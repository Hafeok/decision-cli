---
id: TC-442
title: cell tools declaration intersects with role surface in the dispatch payload
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_174_cell_narrowing_intersection
runner-timeout: 300
observes:
- file
---

## Description

The role surface is seeded as `[read_file, write_file, run_tests]`; the cell declares `tools: [read_file, write_file]`. Asserts on **file** (the stub worker's recorded payload carries `allowed_tools = [read_file, write_file]` — the exact intersection, with `run_tests` absent and nothing added).
