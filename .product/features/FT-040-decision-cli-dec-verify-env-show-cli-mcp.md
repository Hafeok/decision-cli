---
id: FT-040
title: 'decision-cli: dec verify env show (CLI + MCP)'
phase: 2
status: complete
depends-on:
- FT-034
- FT-035
adrs:
- ADR-028
- ADR-029
tests:
- TC-051
- TC-052
- TC-062
domains: []
domains-acknowledged:
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-040 produces no feedback artifacts.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-040's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-040's code is organised under that migration, not by this feature.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-040 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-040 produces no feedback artifacts.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-040 runs after the working directory is resolved and does not re-discover it.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-040 neither emits nor consumes verdicts.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-040 does not author or modify a fitness-function artifact.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-040 is out of scope for the pairing.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-040 does not cross or alter that boundary.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-040 neither emits nor routes feedback.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-040 does not introduce or modify a role catalog entry.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-040 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-040 produces no action/interpretation pair.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-040 has no feedback to gate.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-040 produces no new Session or event type and inherits lineage from the harness.
---

## Description

The `dec verify env show` CLI subcommand and its paired MCP tool `dec_verify_env_show`. Read-only detail view of a single `dec:VerificationEnvironment`. Inherits the single-handler discipline from [ADR-029](ADR-029).

One subcommand → one slice.

## Functional Specification

### Inputs

- CLI form:
  ```
  dec verify env show <ENV-NNN> [--format text|json]
  ```
- MCP form: `dec_verify_env_show` tool with input schema `{ id: string, format?: "text" | "json" }`.
- [FT-035](FT-035)'s env artifact type and named-graph projection.

### Outputs

- CLI default (`--format text`): a multi-line human render — id, env-type, safety-class, endpoint (if any), allowed-ops list, setup/teardown commands (each on its own line, indented). The on-disk path is shown in a trailing footer.
- CLI `--format json`: the full env document as JSON.
- MCP: structured response `{ env: EnvDocument, path: string }` where `EnvDocument` carries every property.

### State

- None. Read-only.

### Behaviour

1. Surface adapter constructs a `Request { id, format? }` for the single handler.
2. Handler resolves the id against the verify-env named graph; absent ids surface as `ArtifactNotFound`.
3. Handler reconstructs the full `VerificationEnvironment` value and resolves the on-disk path.
4. Handler returns `Response { env, path }`.
5. CLI renders text or JSON; MCP returns the structured value.

### Invariants

- Read-only — never modifies state.
- Single handler — CLI and MCP return identical content (modulo rendering).
- The rendered output is round-trip-equivalent to the on-disk Turtle when serialised back.

### Error handling

- Unknown id → `Error::ArtifactNotFound { kind: "VerificationEnvironment", id }`; exit 1.
- Malformed id (does not match `ENV-NNN` pattern) → `Error::InvalidArgument { field: "id", detail }`; exit 2.
- Malformed `--format` value → `Error::InvalidArgument { field: "format", detail }`; exit 2.

### Boundaries

- **In scope.** `dec verify env show` CLI subcommand. `dec_verify_env_show` MCP tool. Two output formats.
- **Out of scope.** Other env subcommands. History / change log (slice 3+).

## Out of scope

- Show history / change log.
- Cross-stream lookup.
- Show by partial id / alias.
