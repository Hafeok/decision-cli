---
id: TC-177
title: Combined dec MCP server exposes both product-cli and decision-cli tool sets without collision
type: scenario
status: unimplemented
validates:
  features:
  - FT-105
  adrs: []
phase: 1
---

## Claim

The combined MCP server exposed by the dec binary registers both product-cli's tool surface AND decision-cli's tool surface on a single endpoint, with no name collisions, no tool dropped from either set, and each tool's behavior identical to its pre-absorption form.

## Scenarios

### Setup

- decision-cli workspace built locally; the `dec mcp` server binary available.
- A reference MCP catalog: the pre-absorption tool list from product-cli (e.g. `product_feature_show`, `product_adr_list`, etc.) and from decision-cli ([FT-034](FT-034)'s tool surface: `dec_verify_graph_generate`, etc.).
- The fixture `.product/` from TC-176.

### Scenario A — tool union is exposed

Invoke `dec mcp list-tools --format json` (or whatever the MCP introspection verb is). Assertions:

- The returned tool list contains **every** tool name from the reference product-cli catalog.
- The returned tool list contains **every** tool name from the reference decision-cli catalog.
- The returned tool list contains **no additional** tools beyond the union of both (no accidental exposure of internal handlers).
- The total count equals `|product_tools| + |decision_tools|`.

### Scenario B — no name collision

Compute the intersection of product-cli's and decision-cli's tool name sets. The test asserts this intersection is **empty**. If a future change introduces a name collision, the dec binary panics at startup (per FT-105 §Invariants); the test must include a sub-assertion that simulates a collision (a test-only registered tool with a conflicting name) and verifies the panic, so the safety net is exercised.

### Scenario C — product-cli tools work identically through the combined server

For a representative subset of product-cli tools (e.g. `product_feature_show`, `product_context`, `product_preflight`):

1. Invoke each through the combined MCP server (`dec mcp call product_feature_show '{"id": "FT-001"}'`).
2. Invoke each through the standalone product-cli MCP server (built from `crates/product-cli/` directly).
3. Assert the returned JSON bodies are structurally identical (using `jq -S .`).

### Scenario D — decision-cli tools work identically through the combined server

Symmetric to Scenario C for a subset of decision-cli MCP tools (e.g. `dec_verify_graph_generate`, `dec_verify_env_list`). Assert structural equality against the existing dec MCP server's responses.

### Scenario E — tool descriptions and schemas pass through unchanged

For each tool in the combined server, the `description` and `inputSchema` fields in the MCP `list-tools` response must match byte-for-byte what the source crate's tool registration declares. The combined server is not allowed to mutate descriptions or schemas in any way. This is asserted by:

1. Loading the source-crate tool definitions (via a small helper that introspects the workspace).
2. Comparing each tool's `description` and serialised `inputSchema` to the combined-server-reported values.

### Scenario F — tool count stable across restarts

Start the combined server, list tools, count. Stop. Start again, list tools, count. Counts must be equal. This catches non-deterministic registration (e.g. registration depending on file-system enumeration order).

### Scenario G — MCP `progress` events work for streaming tools

If either source crate has tools that emit MCP `progress` events (typically long-running ones like `dec_verify_graph_generate`), the test invokes one and asserts that progress events flow through the combined server in the same shape as through the standalone server.

## Runner

`bash tests/scripts/tc-177-combined-mcp.sh`. The script:

1. Builds the workspace.
2. Starts the combined `dec mcp` server in the background, captures its endpoint (stdio or HTTP).
3. Starts a reference standalone product-cli MCP server in the background, captures its endpoint.
4. Runs Scenarios A–G in sequence, asserting on JSON responses.
5. Tears down both servers on exit.

Depends on an MCP client utility being available (a small Python or Rust helper, or `mcp` CLI if such a tool exists in the workspace). The test ships its own minimal client if none is available.

## Non-goals

- Asserting that every tool's implementation is identical (the parity is observable behaviour through the MCP contract, not internal control flow).
- Performance comparison (out of slice).
- The deprecation shim's MCP behavior (the shim is a CLI binary, not an MCP server; this TC is CLI-symmetric scope, not the shim's surface).
- Multi-client concurrent invocation (a separate concern; current MCP servers handle this at the transport layer, untouched by absorption).
