---
id: TC-053
title: dec mcp serve starts and exposes registered tools over stdio
type: exit-criteria
status: unimplemented
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: tc_053_mcp_serve
runner-timeout: 120
---

## Description

[FT-034](FT-034)'s exit criterion: `dec mcp serve` launches an MCP server over stdio, the in-memory registry is populated from feature modules, and an MCP client can complete a `tools/list` handshake. Gracefully shuts down on stdin EOF.

## Acceptance Criteria

1. **Startup.** `dec mcp serve` in a tempdir with `dec init` already run starts, prints nothing to stdout before the first MCP message, and logs a `tracing` startup line to stderr containing `mcp server ready`.

2. **Tools/list handshake.** A test harness issues a JSON-RPC `initialize` followed by `tools/list` over stdio; the response contains every tool registered by feature modules (in slice 2.5, that is every `dec_verify_*` tool from FT-038..FT-044).

3. **Graceful shutdown on EOF.** Closing stdin causes `dec mcp serve` to exit 0 within 1 second; no orphaned threads or temp files remain.

4. **Tool invocation round-trips.** Invoking `dec_verify_env_list` via the MCP server returns the same structured payload as the in-process handler invocation of the same `Request`.

## Fixture

- Tempdir with `dec init --from <seed>.ttl` completed.
- An MCP test client spawning `dec mcp serve` as a subprocess and speaking JSON-RPC over its stdio.

## Out of scope

- Naming convention enforcement (TC-051).
- Duplicate registration (TC-051).
- CLI/MCP twin parity for each subcommand (TC-052).
