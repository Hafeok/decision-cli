---
id: TC-392
title: session list renders cluster cell sessions with parent feature and cellStatus
type: scenario
status: passing
validates:
  features:
  - FT-162
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_162_session_list_cluster tc_392_session_list_renders_cluster_cells_with_parent_feature_and_status
runner-timeout: 120
observes:
- stdout
last-run: 2026-06-05T09:52:39.579981575+00:00
last-run-duration: 0.2s
---

## Description

Scenario test for [FT-162](FT-162) §Outputs branch 2 (cluster cell rendering with parent feature/status). Persists a 2-cell cluster, asserts the cells render with their parent's feature lifted onto the row + cellStatus projected onto the status column.

## Assertions

1. The `agent_loop` cell's row has `feature_id = "FT-T392"` (lifted via `prov:wasInformedBy` → parent → `dec:featureId`).
2. The `agent_loop` cell's row has `status = "succeeded"` (from `dec:cellStatus`).
3. The mechanical `system_prompt` cell's row has the same parent feature and `status = "mechanical"`.

## Runner

`cargo-test` of `tc_392_session_list_renders_cluster_cells_with_parent_feature_and_status`.