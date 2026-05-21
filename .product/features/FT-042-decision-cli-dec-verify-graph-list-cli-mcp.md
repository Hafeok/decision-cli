---
id: FT-042
title: 'decision-cli: dec verify graph list (CLI + MCP)'
phase: 2
status: planned
depends-on:
- FT-034
- FT-036
adrs:
- ADR-028
- ADR-029
tests:
- TC-051
- TC-052
- TC-064
domains: []
domains-acknowledged:
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-042 does not cross or alter that boundary.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-042 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-042 produces no new Session or event type and inherits lineage from the harness.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-042 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-042 runs after the working directory is resolved and does not re-discover it.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-042's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-042 does not author or modify a fitness-function artifact.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-042's code is organised under that migration, not by this feature.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-042 is out of scope for the pairing.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-042 neither emits nor consumes verdicts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-042 produces no action/interpretation pair.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-042 neither emits nor routes feedback.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-042 produces no feedback artifacts.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-042 produces no feedback artifacts.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-042 has no feedback to gate.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-042 does not introduce or modify a role catalog entry.
---

## Description

The `dec verify graph list` CLI subcommand and its paired MCP tool `dec_verify_graph_list`. Read-only listing of all `dec:VerificationGraph` artifacts, with optional filtering by what they verify or which environment they target. Inherits the single-handler discipline from [ADR-029](ADR-029).

One subcommand → one slice.

## Functional Specification

### Inputs

- CLI form:
  ```
  dec verify graph list \
    [--verifies <FT-NNN | TC-NNN>] \
    [--environment <ENV-NNN>] \
    [--format json|table]
  ```
- MCP form: `dec_verify_graph_list` tool with input schema `{ verifies?: string, environment?: string, format?: "json" | "table" }`.
- [FT-036](FT-036)'s graph artifact type and named-graph projection.

### Outputs

- CLI default (`--format table`): table with columns `id`, `verifies`, `environment`, `steps` (count). Empty store prints "no verification graphs yet".
- CLI `--format json`: JSON array of graph summaries.
- MCP: structured response `{ graphs: GraphSummary[] }` where `GraphSummary` is `{ id, verifies, environment, step_count }`.

### State

- None. Read-only.

### Behaviour

1. Surface adapter constructs a `Request` with optional filters.
2. Handler queries the verify-graph named graph via SPARQL, applying filter predicates and computing `step_count` server-side.
3. Handler returns `Response { graphs }`.
4. CLI renders table or JSON; MCP returns the structured value.

### Invariants

- Read-only.
- Single handler.
- Result order is ascending by `VG-NNN` numeric suffix.

### Error handling

- Unknown filter value (malformed FT/TC/ENV id) → `Error::InvalidArgument { field, detail }`; exit 2.
- Store unreadable → `Error::StoreUnreachable`; exit 1.

### Boundaries

- **In scope.** `dec verify graph list` CLI + `dec_verify_graph_list` MCP. Filtering by `verifies` and `environment`, step-count computation, two output formats.
- **Out of scope.** Other graph subcommands. Pagination, watch mode (slice 3+).

## Out of scope

- Pagination.
- Watch mode.
- Cross-stream listing.
- Sorting flags beyond default ascending id.
