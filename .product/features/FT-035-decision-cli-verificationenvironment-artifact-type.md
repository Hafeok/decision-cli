---
id: FT-035
title: 'decision-cli: VerificationEnvironment artifact type'
phase: 2
status: complete
depends-on: []
adrs:
- ADR-028
- ADR-029
tests:
- TC-054
- TC-055
domains: []
domains-acknowledged:
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-035's code is organised under that migration, not by this feature.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-035 neither emits nor consumes verdicts.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-035 is out of scope for the pairing.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-035 produces no action/interpretation pair.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-035 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-035 does not author or modify a fitness-function artifact.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-035 produces no feedback artifacts.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-035 produces no new Session or event type and inherits lineage from the harness.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-035's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-035 does not cross or alter that boundary.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-035 has no feedback to gate.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-035 does not introduce or modify a role catalog entry.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-035 produces no feedback artifacts.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-035 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-035 runs after the working directory is resolved and does not re-discover it.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-035 neither emits nor routes feedback.
---

## Description

Land the `dec:VerificationEnvironment` artifact type — schema, SHACL shape, IRI minting, on-disk raw Turtle persistence at `.dec/verify/env/ENV-NNN.ttl`. Extends the embedded ontology and the `StreamWriter` validation paths. Seeds the `ephemeral-cli` environment at `dec init`.

Substrate consumed by [FT-037](FT-037) (safety enforcement) and by the env CLI subcommand-features ([FT-038](FT-038), [FT-039](FT-039), [FT-040](FT-040)). Pure schema-shaped feature: it does not author any `dec verify env` subcommand by itself.

## Functional Specification

### Inputs

- The embedded ontology (slice 1) — extended here.
- The `StreamWriter` chokepoint — extended to recognise the new shape.
- The `core::vocab` module — gains new IRIs.
- The `dec init` pipeline — extended to seed the default env idempotently.
- The Turtle vocabulary from [ADR-028](ADR-028) §VerificationEnvironment.

### Outputs

- New SHACL shape `dec:VerificationEnvironmentShape` embedded in the ontology bundle, enforcing:
  - `dec:envType` is a non-empty string,
  - `dec:safetyClass` is one of `isolated`, `shared-non-destructive`, `production-readonly`,
  - `dec:allowedOps` is a non-empty rdf:List of operation tokens,
  - `dec:endpoint` is required iff `dec:envType` matches a remote-type pattern (e.g. `remote-http`).
- New IRIs in `core::vocab`:
  - `dec:VerificationEnvironment` (class).
  - `dec:envType`, `dec:setup`, `dec:teardown`, `dec:allowedOps`, `dec:safetyClass`, `dec:endpoint` (properties).
  - `safety:isolated`, `safety:shared-non-destructive`, `safety:production-readonly` (controlled vocabulary literals).
- New Rust types under `core::ontology::verification_env`:
  - `enum SafetyClass { Isolated, SharedNonDestructive, ProductionReadonly }` with `as_str` / `parse`.
  - `struct VerificationEnvironment { id, env_type, setup, teardown, allowed_ops, safety_class, endpoint }`.
  - `fn to_quads(&self, graph: NamedNodeRef) -> Vec<Quad>` — serialises to RDF.
  - `fn from_turtle(path: &Path) -> Result<Self>` — round-trip parse.
- On-disk layout: `.dec/verify/env/ENV-NNN.ttl`. IRI scheme: `https://decision-cli.dev/ns/env/<id>`.
- `dec init` extended to seed `ENV-001-ephemeral-cli.ttl` (safety class `isolated`, allowed ops = `shell`, `filesystem`, `sparql-local`) idempotently — re-running `dec init` does not duplicate the seed.

### State

- One `.ttl` file per environment in `.dec/verify/env/`.
- Named-graph projection in the orchestration store: `https://decision-cli.dev/ns/graph/verify-env`.
- On-disk Turtle is authoritative; the store projection is rebuilt from disk on every load.

### Behaviour

1. Define the SHACL shape; embed in the ontology bundle.
2. Add new IRIs to `core::vocab`.
3. Implement `to_quads` / `from_turtle` for round-trip fidelity.
4. Extend `StreamWriter` so commits including `VerificationEnvironment` quads validate against the shape; SHACL failure produces a structured error and aborts the commit.
5. Add a loader that reads every file under `.dec/verify/env/*.ttl` and projects into the named graph; called at `dec init` and at every command that needs envs.
6. Seed `ephemeral-cli` at `dec init` only if absent.

### Invariants

- Every env has non-empty `dec:envType` and `dec:safetyClass` from the controlled vocabulary.
- `dec:allowedOps` is a non-empty rdf:List of operation tokens.
- Remote env types carry a `dec:endpoint`; local types do not (SHACL conditional).
- On-disk Turtle and in-store projection are kept in sync — on-disk wins on conflict.
- The seeded `ephemeral-cli` env is reproducible: same bytes after every fresh `dec init`.

### Error handling

- SHACL violation on commit → `Error::SchemaViolation { artifact: EnvId, detail }`; CLI exits 1, MCP returns structured error.
- Malformed Turtle on load → `Error::ParseFailure { path, detail }`; surfaces at `dec init` / list / show commands.
- Duplicate `ENV-NNN` id across files → `Error::DuplicateIri { id, paths }`; abort load.

### Boundaries

- **In scope.** SHACL, IRIs, vocab, Rust types, on-disk layout, store projection, seed env at init.
- **Out of scope.** Authoring CLI commands (separate slices: FT-038, FT-039, FT-040). The graph artifact type ([FT-036](FT-036)). Safety enforcement logic ([FT-037](FT-037)). Edit / delete envs (slice 3+).

## Out of scope

- Editing existing envs.
- Deleting envs.
- Versioning envs.
- Remote env health probes.
- Per-stream env scoping.
- Env templates / inheritance.
