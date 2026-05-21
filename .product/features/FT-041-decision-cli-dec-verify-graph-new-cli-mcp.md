---
id: FT-041
title: 'decision-cli: dec verify graph new (CLI + MCP)'
phase: 2
status: planned
depends-on:
- FT-034
- FT-035
- FT-036
- FT-037
adrs:
- ADR-028
- ADR-029
tests:
- TC-051
- TC-052
- TC-063
domains: []
domains-acknowledged:
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-041 does not cross or alter that boundary.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-041 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-041 produces no new Session or event type and inherits lineage from the harness.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-041 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-041 runs after the working directory is resolved and does not re-discover it.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-041's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-041 does not author or modify a fitness-function artifact.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-041's code is organised under that migration, not by this feature.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-041 is out of scope for the pairing.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-041 neither emits nor consumes verdicts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-041 produces no action/interpretation pair.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-041 neither emits nor routes feedback.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-041 produces no feedback artifacts.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-041 produces no feedback artifacts.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-041 has no feedback to gate.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-041 does not introduce or modify a role catalog entry.
---

## Description

The `dec verify graph new` CLI subcommand and its paired MCP tool `dec_verify_graph_new`. Creates an empty `dec:VerificationGraph` artifact pointing at a feature (or TC) and an environment, persists it to `.dec/verify/graph/VG-NNN.ttl`. The graph is authored incrementally: steps are appended via `dec verify step add` ([FT-044](FT-044)). Inherits the single-handler discipline from [ADR-029](ADR-029).

One subcommand → one slice.

## Functional Specification

### Inputs

- CLI form:
  ```
  dec verify graph new [<ID-OR-AUTO>] \
    --verifies <FT-NNN | TC-NNN> \
    --environment <ENV-NNN>
  ```
- MCP form: `dec_verify_graph_new` tool with input schema `{ id?: string, verifies: string, environment: string }`.
- [FT-036](FT-036)'s graph artifact type and SHACL.
- [FT-035](FT-035)'s env type (for the `--environment` reference check).
- [FT-037](FT-037)'s safety enforcement (graph-new creates an empty graph so the safety check is currently a no-op, but the integration point is established here so step-add inherits the wiring).

### Outputs

- A new `.dec/verify/graph/VG-NNN.ttl` file containing the graph header (verifies + environment) and an empty `dec:steps` list.
- A named-graph projection in the orchestration store.
- CLI: prints the minted id and file path; exit 0. MCP: returns `{ id, path }`.

### State

- One new graph file on success. Idempotent only on explicit `--id` with identical canonical content.

### Behaviour

1. Surface adapter constructs a `Request { id?, verifies, environment }`.
2. Handler validates: `verifies` resolves to a known feature or TC (delegated to product-cli's artifact resolution); `environment` resolves to a known `VerificationEnvironment`.
3. Handler mints the next `VG-NNN` id or accepts the caller-provided id (collision-checked).
4. Handler constructs the empty `VerificationGraph` value; serialises via [FT-036](FT-036)'s `to_quads`.
5. Handler commits through `StreamWriter` — SHACL runs. [FT-037](FT-037)'s `check_graph_against_env` is invoked but trivially passes on an empty step list.
6. On success, handler writes the canonical Turtle file to `.dec/verify/graph/`.
7. Handler returns `Response { id, path }`.

### Invariants

- Single-handler discipline per [ADR-029](ADR-029).
- No bypass of `StreamWriter`.
- Empty graphs are valid at create time; the non-empty-steps invariant applies pre-dispatch only (slice 3).
- The graph file is written only after SHACL passes.

### Error handling

- `verifies` reference does not resolve to a feature or TC → `Error::DanglingRef { ref, kind: "verifies" }`; exit 1.
- `environment` reference does not resolve to a known env → `Error::DanglingRef { ref, kind: "environment" }`; exit 1.
- Caller-supplied id collides → `Error::DuplicateId { id }`; exit 1.
- SHACL violation → `Error::SchemaViolation`; exit 1.
- I/O failure → `Error::Io`; exit 1.

### Boundaries

- **In scope.** `dec verify graph new` CLI + `dec_verify_graph_new` MCP. Reference validation, id minting, single-handler write through `StreamWriter`, file persistence.
- **Out of scope.** Other graph subcommands. Step authoring ([FT-044](FT-044)). Bulk graph creation. Inline step authoring at create time (kept separate for slice clarity).

## Out of scope

- Creating a graph with inline steps (use `step add` after).
- Cloning an existing graph.
- Edit graph header (verifies / environment) after create.
- Delete graph.
