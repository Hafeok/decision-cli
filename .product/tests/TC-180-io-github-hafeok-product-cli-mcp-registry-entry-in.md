---
id: TC-180
title: io.github.Hafeok/product-cli MCP registry entry installs a working product binary after FT-106 lands
type: scenario
status: passing
validates:
  features:
  - FT-106
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-180-mcp-registry-backwards-compat.sh
runner-timeout: 120
last-run: 2026-05-28T08:49:13.087442580+00:00
last-run-duration: 0.2s
---

## Claim

The MCP registry entry `io.github.Hafeok/product-cli` continues to install a working `product` MCP server after FT-106 ships. Existing users see only the deprecation warning on the side; the registry name, the install mechanism, the binary name, and the runtime arguments are unchanged.

## Scenarios

### Setup

- A clean machine (or container) with no prior product-cli installation.
- A network connection that can reach the MCP registry and GitHub Releases.
- An MCP client utility (e.g. the `mcp` CLI or `claude` / `inspector`) capable of installing and invoking registry-listed MCP servers.

### Scenario A — fresh install from the legacy registry entry

1. Use an MCP client to install `io.github.Hafeok/product-cli` from the official registry. The exact command depends on the client; for the reference Claude Code MCP installer it's `claude mcp install io.github.Hafeok/product-cli` (or equivalent).
2. The install downloads the MCPB package, extracts the `product` binary, and registers it as a stdio MCP server.

Assertions:
- The install succeeds (exit 0 from the install command).
- A `product` binary exists in the install location.
- The MCP server is registered under the name `io.github.Hafeok/product-cli` (or the client's local alias) and is invokable.

### Scenario B — the installed server responds to `tools/list`

Invoke the installed MCP server (the client may have a verb like `mcp invoke <name> tools/list`). Assertions:
- The server starts.
- A `tools/list` request returns a non-empty list.
- The list contains the same tool names the standalone product-cli MCP server exposed pre-absorption (e.g. `product_feature_show`, `product_adr_list`, `product_context`, etc.). The test ships a `KNOWN_TOOLS` snapshot for assertion; new tools added are allowed, removed tools are a failure.

### Scenario C — invoking a tool produces the expected result

For a small subset of tools (e.g. `product_feature_show` with a fixture feature ID), invoke through the registered server and assert the response matches what the standalone product-cli would have returned for the same input. (The test uses a fixture `.product/` directory.)

### Scenario D — the deprecation warning surfaces

Invoking the `product` binary directly (not through MCP — i.e. `product feature show FT-001` from a shell) prints the deprecation warning to stderr (per FT-105 §Phase 5). Assertions:
- Stderr contains the literal substring `"deprecated"` and a reference to `dec product`.
- Stdout is unaffected — the deprecation warning does NOT contaminate machine-readable output.

### Scenario E — the registry URL pattern resolves

Manually construct the URL from the updated `crates/product-cli/server.json` (after substituting the release version): `https://github.com/Hafeok/decision-cli/releases/download/v<VERSION>/product-x86_64-unknown-linux-gnu.tar.xz`. Assertions:
- `curl --head` against the URL returns HTTP 200 (or 302 → 200).
- The downloaded archive's SHA-256 matches the `fileSha256` published in the registry server.json.

This verifies the publishing pipeline correctly substitutes the URL placeholder and uploads the asset to the absorbed workspace's releases (not the archived standalone product-cli repo).

### Scenario F — existing scripted invocations continue to work

Operators who scripted product-cli's CLI surface (e.g. shell scripts that call `product feature show ... | jq ...`) keep working. Test with a small representative script:

```bash
product feature show FT-001 --format json | jq -r '.id'
```

Assertions:
- Exit 0.
- Stdout contains the feature ID (machine-parseable).
- The deprecation warning is on stderr (doesn't pollute the pipe).

## Runner

`bash tests/scripts/tc-180-mcp-registry-backwards-compat.sh`. The test depends on network access to the MCP registry; runs in a CI environment that can reach it. The script skips with a clear diagnostic if network is unavailable (so local runs without network don't false-fail).

This TC is **integration-level** — it depends on the actual MCP registry, an actual MCP client, and an actual GitHub Release. It runs on a schedule (nightly) and on release-tag pushes, not on every PR. The PR-time gate is TC-181 (schema validation), which is fast.

## Non-goals

- Behaviour of the `dec` MCP server entry (the new one) — that's tested separately via TC-177 (combined MCP server) and a future TC against the registry for `io.github.Hafeok/decision-cli`.
- Cross-platform binary installation — this TC is Linux x86_64 specific because that's what MCPB ships today.
- Performance comparison between the deprecation shim and the standalone product-cli binary (out of slice).
- The transition path for users to migrate from `io.github.Hafeok/product-cli` to `io.github.Hafeok/decision-cli` — there's no migration tooling planned; users opt in on their own timeline.