---
id: FT-039
title: 'decision-cli: dec verify env list (CLI + MCP)'
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
- TC-061
domains: []
domains-acknowledged:
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-039 runs after the working directory is resolved and does not re-discover it.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-039 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-039 produces no action/interpretation pair.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-039 does not introduce or modify a role catalog entry.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-039 has no feedback to gate.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-039 neither emits nor routes feedback.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-039 produces no feedback artifacts.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-039 is out of scope for the pairing.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-039 does not author or modify a fitness-function artifact.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-039 produces no feedback artifacts.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-039 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-039's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-039 neither emits nor consumes verdicts.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-039 produces no new Session or event type and inherits lineage from the harness.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-039's code is organised under that migration, not by this feature.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-039 does not cross or alter that boundary.
---

## Description

The `dec verify env list` CLI subcommand and its paired MCP tool `dec_verify_env_list`. Read-only listing of all `dec:VerificationEnvironment` artifacts known to the orchestration store, with optional filtering by safety class or env type. Inherits the single-handler discipline from [ADR-029](ADR-029).

One subcommand → one slice.

## Functional Specification

### Inputs

- CLI form:
  ```
  dec verify env list \
    [--safety-class <isolated|shared-non-destructive|production-readonly>] \
    [--type <env-type>] \
    [--format json|table]
  ```
- MCP form: `dec_verify_env_list` tool with input schema `{ safety_class?: enum, env_type?: string, format?: "json" | "table" }`.
- [FT-035](FT-035)'s env artifact type and named-graph projection.

### Outputs

- CLI default (`--format table`): table with columns `id`, `type`, `safety-class`, `endpoint`, `allowed-ops` (truncated to fit terminal width). Empty store prints "no environments yet".
- CLI `--format json`: JSON array of env summaries, one per row.
- MCP: structured response `{ envs: EnvSummary[] }` where `EnvSummary` is `{ id, env_type, safety_class, endpoint?, allowed_ops, setup?, teardown? }`.

### State

- None. Read-only.

### Behaviour

1. Surface adapter constructs a `Request` for the single handler with optional filter fields.
2. Handler queries the verify-env named graph via SPARQL `SELECT`, applying filter predicates server-side.
3. Handler returns `Response { envs }` to the surface adapter.
4. CLI renders the table or JSON; MCP returns the structured value.

### Invariants

- Read-only — never modifies state.
- Single handler — CLI and MCP return identical content (modulo rendering).
- Filtering is deterministic — same input always returns the same set in stable id order.
- The order is ascending by `ENV-NNN` numeric suffix.

### Error handling

- Unknown safety-class value → `Error::InvalidArgument { field: "safety_class", detail }`; exit 2.
- Malformed `--format` value → `Error::InvalidArgument { field: "format", detail }`; exit 2.
- Store unreadable → `Error::StoreUnreachable { detail }`; exit 1.

### Boundaries

- **In scope.** `dec verify env list` CLI subcommand. `dec_verify_env_list` MCP tool. Filtering, sorting, two output formats.
- **Out of scope.** Other env subcommands. Pagination (slice 3+). Watch / live updates (slice 3+).

## Out of scope

- Pagination.
- Watch mode / live updates.
- Cross-stream listing (single-stream Phase A).
- Sorting flags beyond default ascending id.
