---
id: FT-038
title: 'decision-cli: dec verify env new (CLI + MCP)'
phase: 2
status: in-progress
depends-on:
- FT-034
- FT-035
adrs:
- ADR-028
- ADR-029
tests:
- TC-051
- TC-052
- TC-060
- TC-094
domains: []
domains-acknowledged:
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-038 runs after the working directory is resolved and does not re-discover it.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-038 does not cross or alter that boundary.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-038 is out of scope for the pairing.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-038 does not author or modify a fitness-function artifact.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-038's code is organised under that migration, not by this feature.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-038 has no feedback to gate.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-038 produces no feedback artifacts.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-038 produces no new Session or event type and inherits lineage from the harness.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-038's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-038 produces no action/interpretation pair.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-038 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-038 neither emits nor routes feedback.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-038 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-038 produces no feedback artifacts.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-038 does not introduce or modify a role catalog entry.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-038 neither emits nor consumes verdicts.
---

## Description

The `dec verify env new` CLI subcommand and its paired MCP tool `dec_verify_env_new`. Creates a new `dec:VerificationEnvironment` artifact, persists it to `.dec/verify/env/ENV-NNN.ttl`, projects into the orchestration store. Inherits the single-handler discipline from [ADR-029](ADR-029) and the artifact type from [FT-035](FT-035). Mints fresh `ENV-NNN` ids; honours caller-supplied ids with collision detection.

One subcommand → one slice per `CLAUDE.md §Discipline within decision-cli` and the user's explicit slice 2.5 rule "each subcommand should be its own slice".

## Functional Specification

### Inputs

- CLI form:
  ```
  dec verify env new [<ID-OR-AUTO>] \
    --type <env-type> \
    --safety-class <isolated|shared-non-destructive|production-readonly> \
    --allowed-ops <comma-separated-ops> \
    [--setup <cmd>] \
    [--teardown <cmd>] \
    [--endpoint <url>]
  ```
- MCP form: `dec_verify_env_new` tool with input schema `{ id?: string, env_type: string, safety_class: enum, allowed_ops: string[], setup?: string, teardown?: string, endpoint?: string }`.
- [FT-035](FT-035)'s `VerificationEnvironment` type and SHACL shape.
- [FT-034](FT-034)'s tool registration pattern and single-handler discipline.

### Outputs

- A new `.dec/verify/env/ENV-NNN.ttl` file containing the env's Turtle.
- A named-graph projection in the orchestration store (one env-worth of quads).
- CLI: prints the minted id and absolute file path; exit 0. MCP: returns `{ id, path }` as the structured tool result.

### State

- One new env file on success. Idempotent only when the caller passes an explicit `--id` matching an existing file with identical canonical content (re-run is a no-op); otherwise a fresh id is minted.

### Behaviour

1. Surface adapter (CLI clap or MCP JSON binding) constructs a `Request` for the single handler.
2. Handler validates inputs: env-type non-empty, safety-class in the controlled list, allowed-ops non-empty, endpoint required iff env-type matches a remote-type pattern (`remote-http`, `remote-grpc`, etc.).
3. Handler mints the next `ENV-NNN` id or accepts the caller-provided id (collision-checked).
4. Handler constructs the `VerificationEnvironment` value; serialises via [FT-035](FT-035)'s `to_quads`.
5. Handler commits through `StreamWriter` — SHACL runs at this chokepoint.
6. On success, handler writes the canonical Turtle file to `.dec/verify/env/`.
7. Handler returns `Response { id, path }` to the surface adapter.

### Invariants

- The CLI and MCP surfaces invoke the **same handler** (single-handler discipline per [ADR-029](ADR-029)).
- No bypass of `StreamWriter` — every env mutation goes through SHACL.
- The Turtle file is written only after SHACL passes (no partial state on failure).
- Id minting is monotonic — minted ids are never reused, even after deletion.

### Error handling

- Invalid input (unknown env-type, malformed allowed-ops, missing endpoint on remote type) → `Error::InvalidArgument { field, detail }`; exit 2 on CLI (usage error), structured error on MCP.
- SHACL violation → `Error::SchemaViolation { detail }`; exit 1 / structured error.
- I/O failure writing the file → `Error::Io { detail }`; exit 1 / structured error.
- Caller-supplied id collides with an existing file containing different content → `Error::DuplicateId { id }`; exit 1 / structured error.

### Boundaries

- **In scope.** `dec verify env new` CLI subcommand. `dec_verify_env_new` MCP tool. Input parsing, id minting, single-handler dispatch, write through `StreamWriter`, file persistence, response rendering.
- **Out of scope.** Other env subcommands (`list`, `show`). The env artifact type itself ([FT-035](FT-035)). Safety enforcement ([FT-037](FT-037) — env new does not yet reference any graph, so the check is a no-op).

## Out of scope

- Edit existing env.
- Delete env.
- Bulk import (multiple envs in one call).
- Env templates / inheritance.
- Interactive prompting.
