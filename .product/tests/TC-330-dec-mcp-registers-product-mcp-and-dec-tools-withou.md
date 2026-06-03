---
id: TC-330
title: dec mcp registers product-mcp and dec tools without name collision
type: scenario
status: passing
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --test ft_136_mcp_merge_no_collision
runner-timeout: 120
observes:
- mcp-response
last-run: 2026-06-03T10:59:22.224178625+00:00
last-run-duration: 0.4s
---

## Acceptance criteria

Verifies that [FT-136](FT-136)'s MCP merge (Phase 3) registers `product_mcp::registry::ToolRegistry`'s tools alongside dec's own without name collision.

### Conditions

- Boot the `dec mcp` stdio server inside an integration test (tokio task or `assert_cmd::Command::stdin_stdout`).
- Send a JSON-RPC `tools/list` request.
- Parse the `result.tools[]` array from the response.
- Assert: the array contains at least one tool whose name starts with `product_` (sourced from `product_mcp`).
- Assert: the array contains at least one tool whose name starts with `dec_` (sourced from dec's own registry).
- Assert: no two entries share the same `name` field — `tools.len() == tools.iter().map(|t| &t.name).collect::<HashSet<_>>().len()`.

### Failure modes covered

- A future `dec_*` tool added with a name that collides with an existing `product_*` tool fails the third assertion.
- A `product_mcp` upgrade that drops a tool (no `product_*` left) fails the first assertion.
- A registry-wiring regression that drops dec's own tools fails the second.

### Surface

`mcp-response` — the test asserts against the live JSON-RPC response, not against a static registry snapshot.