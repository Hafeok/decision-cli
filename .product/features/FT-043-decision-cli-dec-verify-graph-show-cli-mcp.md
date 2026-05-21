---
id: FT-043
title: 'decision-cli: dec verify graph show (CLI + MCP)'
phase: 2
status: complete
depends-on:
- FT-034
- FT-036
adrs:
- ADR-028
- ADR-029
tests:
- TC-051
- TC-052
- TC-065
domains: []
domains-acknowledged:
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-043 produces no feedback artifacts.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-043 runs after the working directory is resolved and does not re-discover it.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-043 does not cross or alter that boundary.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-043 is out of scope for the pairing.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-043 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-043 has no feedback to gate.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-043 neither emits nor routes feedback.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-043 neither emits nor consumes verdicts.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-043 produces no feedback artifacts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-043 produces no action/interpretation pair.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-043 does not author or modify a fitness-function artifact.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-043 does not introduce or modify a role catalog entry.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-043's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-043 produces no new Session or event type and inherits lineage from the harness.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-043 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-043's code is organised under that migration, not by this feature.
---

## Description

The `dec verify graph show` CLI subcommand and its paired MCP tool `dec_verify_graph_show`. Read-only detail view of a single `dec:VerificationGraph` — header (verifies, environment) followed by the ordered list of steps with each step's kind and key fields. Inherits the single-handler discipline from [ADR-029](ADR-029).

One subcommand → one slice.

## Functional Specification

### Inputs

- CLI form:
  ```
  dec verify graph show <VG-NNN> [--format text|json]
  ```
- MCP form: `dec_verify_graph_show` tool with input schema `{ id: string, format?: "text" | "json" }`.
- [FT-036](FT-036)'s graph artifact type and named-graph projection.

### Outputs

- CLI default (`--format text`):
  ```
  VG-NNN
  Verifies:    FT-NNN | TC-NNN
  Environment: ENV-NNN (safety: <class>)
  Steps:
    1. <kind>    <one-line summary of fields>
    2. <kind>    <summary>
    ...
  Path: .dec/verify/graph/VG-NNN.ttl
  ```
- CLI `--format json`: full graph document as JSON, including every step's full field set.
- MCP: structured response `{ graph: GraphDocument, path: string }`.

### State

- None. Read-only.

### Behaviour

1. Surface adapter constructs a `Request { id, format? }`.
2. Handler resolves the id against the verify-graph named graph; absent ids surface as `ArtifactNotFound`.
3. Handler reconstructs the full `VerificationGraph` (header + ordered steps).
4. Handler returns `Response { graph, path }`.
5. CLI renders text or JSON; MCP returns the structured value.

### Invariants

- Read-only.
- Single handler — CLI and MCP return identical content (modulo rendering).
- Step order in the rendered output matches the on-disk rdf:List order exactly.
- Round-trip equivalent — rendering and reserialising yields canonically identical Turtle.

### Error handling

- Unknown id → `Error::ArtifactNotFound { kind: "VerificationGraph", id }`; exit 1.
- Malformed id (does not match `VG-NNN`) → `Error::InvalidArgument { field: "id", detail }`; exit 2.
- Malformed `--format` value → `Error::InvalidArgument { field: "format", detail }`; exit 2.

### Boundaries

- **In scope.** `dec verify graph show` CLI + `dec_verify_graph_show` MCP. Two output formats, header + ordered step rendering.
- **Out of scope.** Other graph subcommands. DAG renderer (slice 3+ — current model is a linear ordered list). History / change log (slice 3+).

## Out of scope

- ASCII DAG renderer (slice 3 — when `${capture}` resolution lands).
- History / change log per graph.
- Diff against another graph.
- Show by partial id / alias.
