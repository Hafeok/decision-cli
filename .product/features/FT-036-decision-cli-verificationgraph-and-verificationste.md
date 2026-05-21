---
id: FT-036
title: 'decision-cli: VerificationGraph and VerificationStep artifact types'
phase: 2
status: complete
depends-on: []
adrs:
- ADR-028
tests:
- TC-056
- TC-057
domains: []
domains-acknowledged:
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-036 does not introduce or modify a role catalog entry.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-036 has no feedback to gate.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-036 is out of scope for the pairing.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-036 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-036 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-036 produces no new Session or event type and inherits lineage from the harness.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-036's code is organised under that migration, not by this feature.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-036 neither emits nor routes feedback.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-036 produces no feedback artifacts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-036 produces no action/interpretation pair.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-036 produces no feedback artifacts.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-036 neither emits nor consumes verdicts.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-036 runs after the working directory is resolved and does not re-discover it.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-036 does not author or modify a fitness-function artifact.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-036's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-036 does not cross or alter that boundary.
---

## Description

Land the `dec:VerificationGraph` and `dec:VerificationStep` artifact types — top-level graph shape, ordered list of steps, SHACL shapes for the 6 seed step types per [ADR-028](ADR-028). On-disk raw Turtle at `.dec/verify/graph/VG-NNN.ttl`. Reserves the `${name}` capture syntax (unresolved in slice 2.5).

Substrate consumed by [FT-037](FT-037) (safety enforcement) and the graph / step CLI subcommand-features ([FT-041](FT-041), [FT-042](FT-042), [FT-043](FT-043), [FT-044](FT-044)).

## Functional Specification

### Inputs

- The embedded ontology — extended here.
- The `StreamWriter` chokepoint — extended for graph + step shapes.
- The `core::vocab` module — gains graph / step IRIs.
- [ADR-028](ADR-028)'s step-type vocabulary and `dec:requiredOps` declarations.

### Outputs

- SHACL shapes:
  - `dec:VerificationGraphShape` — requires `dec:verifies`, `dec:environment`; `dec:steps` is an rdf:List (may be empty during authoring; non-emptiness is a pre-dispatch invariant enforced in slice 3, not at persistence).
  - One step-type shape per seed type: `dec:ShellCommandStepShape`, `dec:SparqlAssertionStepShape`, `dec:FileAssertionStepShape`, `dec:HttpRequestStepShape`, `dec:WaitForStepShape`, `dec:CaptureStepShape`.
  - Each step shape declares its `dec:requiredOps` (machine-readable subset consumed by [FT-037](FT-037)'s safety check).
- `dec:verifies` is polymorphic — SHACL accepts `dec:Feature` or `dec:TC` as the object class.
- New IRIs in `core::vocab`:
  - `dec:VerificationGraph`, `dec:VerificationStep` (classes).
  - `dec:verifies`, `dec:environment`, `dec:steps`, `dec:stepType`, `dec:requiredOps` (graph + step structural properties).
  - Per-step-type fields: `dec:command`, `dec:expectExitCode`, `dec:captureOutput`, `dec:target`, `dec:query`, `dec:expectRows`, `dec:path`, `dec:expectHash`, `dec:method`, `dec:url`, `dec:expectStatus`, `dec:condition`, `dec:timeout`, `dec:bindAs`.
  - `dec:providesEvidenceFor` (per [ADR-028](ADR-028)'s coverage-predicate section) — optional on every step kind, range is `dec:TC`, cardinality zero-or-more. Consumed by [FT-045](FT-045) (coverage primitive), [ADR-030](ADR-030)'s verify-graph-author, and [ADR-031](ADR-031)'s chain-integrity gate ([FT-047](FT-047)).
- Rust types under `core::ontology::verification_graph`:
  - `enum StepKind { ShellCommand, SparqlAssertion, FileAssertion, HttpRequest, WaitFor, Capture }`.
  - `enum StepFields { ShellCommand { command, expect_exit_code, capture_output }, SparqlAssertion { target, query, expect_rows }, FileAssertion { path, expect_hash, expect_content }, HttpRequest { method, url, expect_status }, WaitFor { condition, timeout }, Capture { from_step, bind_as } }` — discriminated union mirroring the SHACL shapes.
  - `struct VerificationStep { id: StepIri, kind: StepKind, fields: StepFields }`.
  - `struct VerificationGraph { id: GraphIri, verifies: ArtifactRef, environment: EnvIri, steps: Vec<VerificationStep> }`.
  - `fn to_quads(&self, named_graph: NamedNodeRef) -> Vec<Quad>` / `fn from_turtle(path: &Path) -> Result<Self>` for round-trip.
- On-disk layout: `.dec/verify/graph/VG-NNN.ttl`. IRI scheme: `https://decision-cli.dev/ns/graph/<id>`; steps as `https://decision-cli.dev/ns/step/<graph-id>/<index>`.
- `${name}` placeholder strings in step bodies are preserved verbatim; this feature does not resolve them.

### State

- One `.ttl` file per graph under `.dec/verify/graph/`.
- Named-graph projection in orchestration store: `https://decision-cli.dev/ns/graph/verify-graph`.
- On-disk Turtle is authoritative; store projection is rebuilt from disk on every load.

### Behaviour

1. Define SHACL shapes and embed in the ontology bundle.
2. Extend `core::vocab` with the new IRIs.
3. Implement Rust types and round-trip serialisers per step kind.
4. Extend `StreamWriter` to validate `VerificationGraph` and `VerificationStep` quads — the step-kind-specific shape is selected by the step's `dec:stepType` literal.
5. Loader reads `.dec/verify/graph/*.ttl` and projects into the store; on-disk wins on conflict.
6. Preserve `${name}` references verbatim — no interpretation, no validation against capture availability.

### Invariants

- Every graph has `dec:verifies` pointing at a `dec:Feature` or `dec:TC`, and `dec:environment` pointing at a `dec:VerificationEnvironment`.
- Every step has `dec:stepType` in the seed vocabulary; the step-kind-specific shape gates its other properties.
- Every step shape carries an OPTIONAL `dec:providesEvidenceFor` predicate (range `dec:TC`, multi-valued); SHACL accepts zero or more values. Coverage tooling ([FT-045](FT-045)) interprets absence as "this step covers no TC", not as an error.
- Step order is the rdf:List order in `dec:steps`; loaders and serialisers preserve it.
- `${name}` placeholder strings appear verbatim in serialised step bodies; this feature does not resolve them and does not warn about unresolved references.
- Step IRIs are derived deterministically from `(graph_id, index)` — append-only authoring produces stable IRIs.

### Error handling

- SHACL violation on commit → `Error::SchemaViolation { artifact: GraphId | StepId, detail }`.
- Malformed Turtle on load → `Error::ParseFailure { path, detail }`.
- `dec:verifies` pointing at neither a known feature nor a known TC → `Error::DanglingRef { ref, kind: "verifies" }`.
- `dec:environment` pointing at an unknown env → `Error::DanglingRef { ref, kind: "environment" }`.
- Unknown `dec:stepType` literal → `Error::UnknownStepKind { value }`.

### Boundaries

- **In scope.** SHACL shapes for graph + each step kind, IRI scheme, Rust types, on-disk layout, store projection, step-kind discriminated union, polymorphic `dec:verifies`, `${name}` literal preservation, `dec:requiredOps` declarations per step kind, optional `dec:providesEvidenceFor` predicate on every step shape.
- **Out of scope.** Authoring CLI commands (separate slices). Safety op-subset check ([FT-037](FT-037) — this feature *declares* `requiredOps` per step kind; FT-037 *enforces* the subset). `${name}` resolution (slice 3). Step executors (slice 3). Pre-dispatch non-empty-steps check (slice 3).

## Out of scope

- Resolving `${name}` capture references.
- Step executors.
- Extension step types (`dagger-pipeline`, `git-state`, `metric-window`, `llm-judgment`).
- Edit / delete graph operations.
- Step removal / reorder.
- Non-linear DAG edges between steps (linear ordered list only in slice 2.5).
