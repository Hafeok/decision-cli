---
id: TC-064
title: dec verify graph list returns graphs with step counts and respects filters
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
---

## Description

[FT-042](FT-042)'s `dec verify graph list` and `dec_verify_graph_list` return all graphs in ascending `VG-NNN` order, include a per-graph step count, and respect `--verifies` and `--environment` filters.

## Acceptance Criteria

1. **Empty store.** With no graphs authored, the CLI prints "no verification graphs yet"; JSON returns `[]`; MCP returns `{ "graphs": [] }`.

2. **Order.** Graphs `VG-001`, `VG-002`, `VG-003` are returned in ascending order regardless of authoring order.

3. **Step count.** A graph with three steps reports `step_count: 3`. An empty graph reports `step_count: 0`. The count is computed server-side via SPARQL.

4. **Filter by verifies.** `--verifies FT-001` returns only graphs whose `dec:verifies` resolves to that feature. `--verifies TC-013` returns only graphs whose `dec:verifies` resolves to that TC.

5. **Filter by environment.** `--environment ENV-001-ephemeral-cli` returns only graphs targeting that env.

6. **Combined filters.** Both filters applied conjunctively.

7. **MCP parity.** Equivalent JSON input returns the same set in the same order; the structured `Response.graphs` matches the CLI JSON output element-for-element.

## Fixture

- Tempdir with at least three graphs covering multiple features/TCs and multiple envs.

## Out of scope

- Graph detail view (TC-065).
- Pagination.
