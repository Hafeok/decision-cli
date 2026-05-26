---
id: ADR-029
title: CLI and MCP pairing for dec content management
status: accepted
features:
- FT-099
- FT-101
- FT-102
- FT-105
supersedes: []
superseded-by: []
domains: []
scope: domain
content-hash: sha256:742b0a4d7495271aeebdff7aecabe4bd78a39085a66d197acb11d5ae5511a8b8
---

## Context

product-cli ships every content-management subcommand as both a CLI command and an MCP tool — `product feature new` is callable from a shell, and `mcp__product__product_feature_new` is callable from an LLM agent. The two transports route to the same handler, so a human and an agent collaborating on the same artifact use identical vocabulary, identical validation, and identical error shapes.

dec inherits this collaboration model — `CLAUDE.md §The principle that governs everything` explicitly names product-cli as the authoring surface for engineering work — but dec itself does not yet expose an MCP server. Slice 1's CLI surface (`dec init`, `dec status`, `dec implement`, `dec events`, `dec session`, `dec health`) is shell-only.

As slice 2.5 introduces `dec verify` with many content-management subcommands (env new/list/show, graph new/show/list, step add), the question is whether to ship them as CLI-only or pair each with an MCP tool from day one.

Shipping CLI-only would split the dec authoring surface: humans get all `dec verify` commands; agents get none, until a later effort retrofits MCP tools onto stable handlers. That retrofit historically goes one of two ways — duplicated implementations that drift, or a thin wrapper layer that mismatches the CLI's argument shapes. Both are avoidable by pairing the surfaces from the outset.

## Decision

**Every `dec` subcommand that manages content ships with a paired MCP tool. Both surfaces route to a single handler.**

The rule applies to:

- **Authoring** commands (`new`, `add`, `body`) — create artifacts.
- **Linking** commands (`depends-on`, `link`, `verifies`) — mutate relationships.
- **Inspection** commands (`list`, `show`) — read artifacts.
- **Deletion** commands (`delete`, `remove`) — remove artifacts.

Read-only operational commands (`dec events tail`, `dec session log`, `dec health`) also pair where the agent flow benefits — for instance, `dec session show <iri>` reads naturally from an agent walking through a verification trace.

### MCP server

A new top-level subcommand `dec mcp serve` runs an MCP server over stdio. The server registers every paired tool at startup. The `dec` binary continues to expose the CLI; `dec mcp serve` is the explicit entry point for the MCP transport.

### Single-handler discipline

Each subcommand-feature exports three pieces:

1. A clap subcommand spec (consumed by `main.rs` for the CLI surface).
2. An MCP tool descriptor (consumed by `dec mcp serve` for the MCP surface).
3. **A single handler function** with a `Request` / `Response` shape both surfaces invoke.

The handler does all validation, all writes through `StreamWriter`, all SHACL gating, all safety checks. Surface-specific code is limited to:

- **Argument parsing**: CLI uses clap; MCP binds the JSON input against the tool's input schema. Both produce the same handler `Request`.
- **Response rendering**: CLI emits human-friendly text to stdout; MCP returns the structured `Response` as the tool result. Both pull from the same source value.

Validation, business logic, and persistence live in the handler — never in the surface adapters.

### Naming convention

MCP tools mirror product-cli's `mcp__product__product_<noun>_<verb>` shape:

- `mcp__dec__dec_verify_env_new`
- `mcp__dec__dec_verify_env_list`
- `mcp__dec__dec_verify_graph_show`
- `mcp__dec__dec_verify_step_add`

The CLI equivalent is the same path with spaces instead of underscores:

```bash
dec verify env new ENV-NNN --type ephemeral-tempdir ...
```

The tool registration layer enforces the naming: a feature attempting to register a tool whose name does not match `dec_<noun>_<verb>` is rejected at server startup.

### Errors

Both surfaces return the same structured error type with stable codes. CLI renders to stderr with an exit code; MCP returns the structured error as the tool result. Adding a new error case adds it on both surfaces simultaneously — there is no surface-specific error code.

## Rejected alternatives

- **CLI-only initially, retrofit MCP later.** Rejected — historical drift. Single-handler discipline only works if both surfaces are designed together; retrofit invariably produces parallel implementations.
- **MCP server in a separate binary.** Rejected — splits the dec install. `dec mcp serve` keeps one binary, one config-discovery path, one set of feature flags.
- **MCP server scoped to verify subcommands only.** Rejected — the principle (CLI ⇔ MCP parity) is cross-cutting, not verify-specific. Future content-management subcommands (subscriptions, role catalog edits if exposed, etc.) inherit the pairing automatically.
- **Per-feature MCP tool registry (decentralised).** Rejected — central registration in `dec mcp serve` keeps the tool list discoverable and the wiring layer thin. Each feature module exports a `ToolDescriptor`; the server module aggregates them — same pattern as clap subcommands in `main.rs`.
- **Pair only authoring commands; leave inspection CLI-only.** Rejected — LLM agents need read access at least as much as write access; inspection pairing is what makes a verifier role usable autonomously.

## Consequences

**Positive:**

- LLM agents and humans get identical authoring power from day one.
- Single handler per subcommand: validation, SHACL, error shape all coherent across surfaces.
- Drift between CLI and MCP is structurally impossible (one handler).
- product-cli's existing patterns (tool naming, handler shape) carry over with no translation tax.
- The MCP surface becomes the natural integration point for LLM-driven authoring in the broader DDD substrate.

**Negative / accepted costs:**

- Every content-management feature has two surface specs to author (clap + MCP descriptor). Bounded: MCP descriptors are typically 5–10 lines.
- The MCP server module is new infrastructure to maintain. The Rust MCP SDK choice (e.g. `rmcp`) is pinned by the first feature implementing this rule.

**Enforcement:**

- A future structural TC asserts every content-management `clap::Command` registered in `main.rs` has a matching MCP tool registered in `dec mcp serve`.
- The tool registry rejects tool names not matching `dec_<noun>_<verb>` at startup.
- The first feature implementing this rule lands the scaffolding; subsequent verify subcommand-features cite this ADR and ship both surfaces.

## Status

Proposed. Cross-cuts every content-management feature in dec. First implemented by the `dec` MCP server scaffolding feature; inherited by every `dec verify` subcommand-feature in slice 2.5 and by every future authoring command in dec.
