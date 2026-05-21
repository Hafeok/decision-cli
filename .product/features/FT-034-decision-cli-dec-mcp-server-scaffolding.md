---
id: FT-034
title: 'decision-cli: dec MCP server scaffolding'
phase: 2
status: complete
depends-on: []
adrs:
- ADR-029
tests:
- TC-051
- TC-053
domains: []
domains-acknowledged:
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-034's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-034's code is organised under that migration, not by this feature.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-034 neither emits nor routes feedback.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-034 runs after the working directory is resolved and does not re-discover it.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-034 does not introduce or modify a role catalog entry.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-034 is out of scope for the pairing.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-034 does not author or modify a fitness-function artifact.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-034 produces no feedback artifacts.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-034 neither emits nor consumes verdicts.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-034 produces no feedback artifacts.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-034 has no feedback to gate.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-034 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-034 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-034 does not cross or alter that boundary.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-034 produces no action/interpretation pair.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-034 produces no new Session or event type and inherits lineage from the harness.
---

## Description

The `dec` MCP server scaffolding. Establishes the `dec mcp serve` subcommand, the tool-registration pattern, and the single-handler discipline that every content-management feature inherits per [ADR-029](ADR-029). Pure substrate — no business logic of its own.

This is the first feature implementing [ADR-029](ADR-029); every subsequent `dec verify` subcommand-feature in slice 2.5 ships its MCP twin against this scaffolding.

## Functional Specification

### Inputs

- The slice-1 `dec` binary (entry point).
- A Rust MCP SDK dependency (pinned at implementation time; `rmcp` or equivalent — recorded in the implementing commit).
- The single-handler discipline from [ADR-029](ADR-029).

### Outputs

- New subcommand `dec mcp serve` that runs an MCP server over stdio.
- A `core::mcp` module exposing:
  - `ToolDescriptor` carrying name, description, JSON input schema, output schema, and a handler reference.
  - `register_tool(ToolDescriptor)` for feature modules to call at startup.
  - `serve_stdio()` entry point invoked by the `dec mcp serve` subcommand.
- A `core::handler::{Request, Response, Error}` shape both surfaces (CLI and MCP) route through.
- Tool naming convention enforced at registration: `dec_<noun>_<verb>` (e.g. `dec_verify_env_new`); registration rejects malformed names with a startup failure.

### State

- No persistent state. An in-memory tool registry is built at startup from feature modules.

### Behaviour

1. `dec mcp serve` starts the MCP server bound to stdio per MCP transport conventions.
2. At startup, the server collects every `ToolDescriptor` registered by feature modules. Registration uses a slice-respecting mechanism (`inventory` crate or equivalent) so feature modules do not import `core::mcp::registry` internals — the slice-level SDP boundary holds.
3. Tool invocations are routed to the descriptor's handler. The handler returns `Result<Response, Error>`; both arms map to the MCP wire protocol's success/error envelopes.
4. The server logs to stderr via the `tracing` crate; stdout is reserved for the MCP wire protocol.
5. SIGINT or EOF on stdin shuts the server down gracefully (in-flight handlers complete, then exit 0).

### Invariants

- The MCP server is a transport — no business logic, no validation, no SHACL.
- Every registered tool has a matching CLI subcommand (one handler, two surfaces).
- Tool names follow `dec_<noun>_<verb>` exactly; registration rejects malformed names.
- The CLI surface and the MCP surface never diverge: changes to a handler affect both transports identically.

### Error handling

- MCP protocol parse error → standard MCP error envelope returned to the caller.
- Tool handler returns `Error` → MCP tool error result on the MCP side; CLI prints to stderr with exit code on the CLI side.
- Duplicate tool name at registration → server startup fails with exit 1 and a diagnostic on stderr.
- SDK runtime error → server logs and exits 1.

### Boundaries

- **In scope.** `dec mcp serve` subcommand, `core::mcp` module, tool registration pattern, `Request`/`Response`/`Error` discipline, naming enforcement.
- **Out of scope.** Any specific tool implementations (those land in subcommand-features). Auth, alternative transports (SSE, websocket), MCP resources, MCP prompts — all slice 3+.

## Out of scope

- MCP resources (slice 3+).
- MCP prompts (slice 3+).
- Server-side caching or batching.
- Alternative transports beyond stdio.
- Tool deprecation / versioning machinery.
- Multi-process concurrency beyond what the underlying SDK provides.
