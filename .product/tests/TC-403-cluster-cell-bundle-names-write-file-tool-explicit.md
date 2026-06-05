---
id: TC-403
title: cluster cell bundle names write_file tool explicitly in numbered workflow
type: exit-criteria
status: passing
validates:
  features:
  - FT-165
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_165_bundle_requires_write_file_tool_call_explicitly
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T11:36:20.721291363+00:00
last-run-duration: 0.5s
---

## Description

Exit-criteria test for [FT-165](FT-165) §Outputs — pins the explicit-tool-call instruction shape in `build_cell_bundle`. A worker reading this bundle has no ambiguity about which tool to call.

## Assertions

The bundle string contains:
1. `### Required workflow` heading.
2. `Call the `write_file` tool with:` — names the tool explicitly.
3. `` `path`: `emitter.rs` `` — pins the target path argument.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_165_bundle_requires_write_file_tool_call_explicitly`.