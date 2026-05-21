---
id: TC-051
title: Every registered MCP tool follows dec_noun_verb naming
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: bash
runner-args: tests/scripts/tc-051-mcp-naming.sh
runner-timeout: 180
last-run: 2026-05-21T13:12:47.703888424+00:00
last-run-duration: 0.6s
---

## Description

[ADR-029](ADR-029) mandates that every dec MCP tool name follows `dec_<noun>_<verb>` (e.g. `dec_verify_env_new`). [FT-034](FT-034)'s registry enforces this at startup. This TC asserts the rule holds across the live tool set — both the structural enforcement (registration rejects malformed names) and the population (every tool currently registered conforms).

## Acceptance Criteria

1. **Naming enforcement.** A unit test attempts to register a `ToolDescriptor` with name `bad name` (space) and another with name `verify_env_new` (missing `dec_` prefix); registration returns an error in both cases.
2. **Live registry conformance.** With `dec mcp serve` invoked in a tempdir, the MCP `tools/list` response contains tools whose names all match the regex `^dec_[a-z]+(_[a-z]+)*$` and start with `dec_`.
3. **Duplicate registration.** Two tools attempting to register the same name cause `dec mcp serve` to exit 1 at startup with the diagnostic naming the duplicated tool.

## Fixture

- A tempdir with `dec init --from <seed>.ttl` completed so the server has a store to attach to.
- A test harness that spawns `dec mcp serve` over stdio and issues an MCP `tools/list` request.

## Out of scope

- Per-tool input schema validity (covered per-feature).
- Tool execution semantics (covered per-feature).