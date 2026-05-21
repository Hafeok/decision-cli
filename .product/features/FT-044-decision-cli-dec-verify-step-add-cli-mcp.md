---
id: FT-044
title: 'decision-cli: dec verify step add (CLI + MCP)'
phase: 2
status: planned
depends-on:
- FT-034
- FT-036
- FT-037
adrs:
- ADR-028
- ADR-029
tests:
- TC-051
- TC-052
- TC-066
- TC-067
domains: []
domains-acknowledged:
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-044 does not cross or alter that boundary.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-044 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-044 produces no new Session or event type and inherits lineage from the harness.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-044 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-044 runs after the working directory is resolved and does not re-discover it.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-044's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-044 does not author or modify a fitness-function artifact.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-044's code is organised under that migration, not by this feature.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-044 is out of scope for the pairing.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-044 neither emits nor consumes verdicts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-044 produces no action/interpretation pair.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-044 neither emits nor routes feedback.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-044 produces no feedback artifacts.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-044 produces no feedback artifacts.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-044 has no feedback to gate.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-044 does not introduce or modify a role catalog entry.
---

## Description

The `dec verify step add` CLI subcommand and its paired MCP tool `dec_verify_step_add`. Appends a typed `dec:VerificationStep` to an existing `dec:VerificationGraph`. Type-discriminated via `--type`; per-type field validation comes from [FT-036](FT-036)'s SHACL shapes. Triggers [FT-037](FT-037)'s safety check against the graph's environment before persistence. Inherits the single-handler discipline from [ADR-029](ADR-029).

One subcommand → one slice. The step kind is a parameter, not a separate subcommand — each kind's validation lives in its SHACL shape (slice substrate), so a single CLI verb is the correct grain.

## Functional Specification

### Inputs

- CLI form:
  ```
  dec verify step add <VG-NNN> \
    --type <shell-command | sparql-assertion | file-assertion | http-request | wait-for | capture> \
    [--field key=value] ...
  ```
  The accepted `--field` keys are per-type (e.g. `--field command="dec init"` for `shell-command`; `--field query="SELECT ..."` for `sparql-assertion`).
- MCP form: `dec_verify_step_add` tool with input schema:
  ```json
  {
    "graph_id": "string",
    "step_type": "shell-command | sparql-assertion | file-assertion | http-request | wait-for | capture",
    "fields": { "<per-type-key>": "<value>" }
  }
  ```
- [FT-036](FT-036)'s graph + step types and per-step-kind SHACL shapes.
- [FT-037](FT-037)'s `check_step_against_env` for the safety check.
- [FT-035](FT-035)'s env (resolved indirectly via the graph's `dec:environment`).

### Outputs

- One new step appended to the graph's `dec:steps` rdf:List.
- The graph's `.ttl` file is rewritten canonically.
- The store projection is updated.
- CLI: prints the minted step id and 1-based position; exit 0. MCP: returns `{ step_id, position }`.

### State

- One new step in the existing graph file. The graph file is rewritten atomically (write to `.tmp`, then rename).

### Behaviour

1. Surface adapter constructs a `Request { graph_id, step_type, fields }`.
2. Handler resolves the graph; absent graph surfaces as `ArtifactNotFound`.
3. Handler validates `step_type` against the seed vocabulary.
4. Handler validates type-specific fields per the corresponding SHACL shape (e.g. `shell-command` requires `command`; `sparql-assertion` requires `target` + `query` + `expect_rows`).
5. Handler mints the step IRI deterministically (`step:<graph-id>/<next-index>`).
6. Handler runs [FT-037](FT-037)'s `check_step_against_env` against the graph's `dec:environment`; refuses on `SafetyViolation`.
7. Handler appends the step to the graph's `dec:steps` list, commits through `StreamWriter` (SHACL re-validates the whole graph), then rewrites the on-disk `.ttl`.
8. Handler returns `Response { step_id, position }`.

### Invariants

- Step order is append order; position is monotonic per graph.
- Safety check runs every step-add — the graph is never persisted in a state that violates safety.
- `${name}` references in field values are preserved verbatim; no resolution and no validation against capture availability.
- Single-handler discipline per [ADR-029](ADR-029).
- The on-disk file is rewritten only after SHACL and safety both pass.

### Error handling

- Unknown `step_type` → `Error::InvalidArgument { field: "step_type", detail }`; exit 2.
- Missing required per-type field (e.g. `command` on `shell-command`) → `Error::SchemaViolation { detail }`; exit 1.
- Unknown per-type field key → `Error::InvalidArgument { field: "fields.<key>", detail }`; exit 2.
- Graph not found → `Error::ArtifactNotFound { kind: "VerificationGraph", id }`; exit 1.
- Safety violation → `Error::SafetyViolation { step, missing_ops, env_allowed_ops, env_safety_class }`; exit 1.
- I/O failure rewriting the file → `Error::Io { detail }`; exit 1.

### Boundaries

- **In scope.** `dec verify step add` CLI + `dec_verify_step_add` MCP. Type-discriminated validation, safety-check integration, append-to-list semantics, atomic file rewrite.
- **Out of scope.** Other step verbs (remove, reorder — slice 3+). `${name}` resolution ([FT-036](FT-036) reserves the syntax; slice 3 resolves it). Non-linear DAG edges between steps (slice 3+). Step executors (slice 3).

## Out of scope

- Remove step.
- Reorder steps.
- Edit existing step fields.
- Non-linear DAG edges between steps (slice 3+ when `${capture}` lands).
- Inline step composition (e.g. macro steps).
