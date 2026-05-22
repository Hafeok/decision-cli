---
id: TC-052
title: Every dec verify subcommand has paired CLI and MCP twin sharing one handler
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test tc_052_every_dec_verify_subcommand_has_paired_cli_and_mcp
runner-timeout: 120
last-run: 2026-05-22T11:32:27.628246642+00:00
last-run-duration: 0.5s
---

## Description

[ADR-029](ADR-029) mandates that every `dec` content-management subcommand has a paired MCP tool routed through a single handler — no parallel implementations. This TC asserts the structural property across every `dec verify` subcommand in slice 2.5.

Two halves: (a) every clap subcommand under `dec verify` has a registered MCP tool, and (b) the two surfaces route to the same handler symbol (no surface-specific business logic).

## Acceptance Criteria

1. **Surface symmetry — every CLI subcommand has an MCP tool.** Enumerate every leaf clap subcommand under the `dec verify` tree; for each, the in-memory MCP tool registry contains a tool whose name matches the path with `_` separators (e.g. `dec verify env new` ⇒ `dec_verify_env_new`). The set difference in either direction is empty.

2. **Single handler — no parallel implementations.** For each (subcommand, tool) pair, the clap handler call site and the MCP descriptor's handler reference resolve to the same function symbol via a `core::handler` registration. A grep-based structural test asserts no `features/ft_*_verify*` module contains two distinct handler entry points (i.e. no `cli_handle` + `mcp_handle` split).

3. **Identical Request shape.** For each pair, a unit test constructs an equivalent `Request` from the CLI args parsed by clap and from the MCP JSON input bound to the tool schema, and asserts the two `Request` values are equal.

4. **Identical Response and Error.** A unit test invokes the handler with a fixture input through both surfaces and asserts the structured `Response` (or `Error`) values are byte-equal modulo surface-specific rendering.

## Fixture

- A test harness building the full `dec` clap tree and the MCP tool registry against a tempdir-backed orchestration store.
- One representative input per subcommand (e.g. `env new`: `--type ephemeral-tempdir --safety-class isolated --allowed-ops shell,filesystem`).

## Out of scope

- Per-subcommand semantic behaviour (covered by each FT-038..FT-044's own TC).
- The MCP transport layer beyond the tool-registry contract (covered by TC-051).