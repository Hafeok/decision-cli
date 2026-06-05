---
id: TC-406
title: cluster cell bundle pins dispatch incomplete until write_file tool call returns
type: scenario
status: passing
validates:
  features:
  - FT-165
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_165_bundle_emphasises_dispatch_incomplete_until_tool_call
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T11:36:20.721291363+00:00
last-run-duration: 0.5s
---

## Description

Scenario test for [FT-165](FT-165) §Behaviour — the bundle frames the dispatch as INCOMPLETE until the tool call succeeds, and surfaces the explicit failure-modes list. Together these framings force the worker to internalize "tool call = success, anything else = failure".

## Assertions

The bundle contains:
1. `dispatch is INCOMPLETE until your `write_file` tool call returns success` — the dispatch-incompleteness invariant.
2. `### Failure modes to avoid` — the failure-modes section heading.
3. `never calling `write_file`` — explicit mention of the dominant failure mode witnessed on FT-147.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_165_bundle_emphasises_dispatch_incomplete_until_tool_call`.