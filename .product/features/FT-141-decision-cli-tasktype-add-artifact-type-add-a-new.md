---
id: FT-141
title: 'decision-cli: TaskType add-artifact-type — add a new typed ontology artifact (Rust struct + SHACL + parser + emitter + round-trip tests)'
phase: 5
status: planned
depends-on: []
adrs:
- ADR-080
- ADR-072
tests:
- TC-354
- TC-355
- TC-356
- TC-357
domains:
- api
domains-acknowledged:
  observability: 'FT-141 authors the add-artifact-type TaskType declaration with 4 TCs (TC-A exit-criteria for topo-order plus TC-B/C/D scenarios over the coherence audit''s behaviour). ADR-072 spans api + observability. Observability concerns are covered by the audit script''s structural checks themselves: each of the 6 audit checks (shacl-field-coverage, iri-const-reachability, parser-field-coverage, emitter-field-coverage, round-trip-tests-both-cases, no-python-files) surfaces as a named, operator-readable check identifier in the ClusterAuditFailed { check, detail } outcome. That outcome is observable in dec drive history and maps back to a specific drift mode without grepping. TC-C asserts the audit failure surfaces with the specific shacl-field-coverage identifier; TC-D asserts the no-python-files identifier; the check name set is the observability surface of the cluster''s contract integrity. Explicit acknowledgement per ADR-072 review gate, mirroring FT-139''s treatment.'
---

## Description

The TaskType feature_spec for **`add-artifact-type`** — the second TaskType slotted into the catalog established by [FT-139](FT-139) under [ADR-080](ADR-080)'s decision to apply DDD task/cell decomposition to decision-cli's self-implementation pipeline.

Witnessed motivating examples — shipped features whose diffs collapse into a single near-identical signature:

| FT | Artifact added | Witnessed shape |
|---|---|---|
| [FT-026](FT-026) | `Feedback` | Rust struct + SHACL shape + parser + emitter + round-trip tests |
| [FT-035](FT-035) | `VerificationBench` | same |
| [FT-054](FT-054) | `Capability` | same |
| [FT-071](FT-071) | `BoundaryArtifact` class + `external_origin` field | same |
| [FT-086](FT-086) | `WorkerImage` | same |

Five shipped features, one shape: define a Rust struct with named fields, declare a SHACL shape with one `sh:property` per field, write a parser that decodes quads into the struct, write an emitter that encodes the struct into quads, write round-trip tests that prove emit-then-parse preserves equality and that SHACL rejects malformed instances. Per ADR-080 §Decision §5, this is exactly the pattern where the cell-cluster decomposition pays the most: the contract surface across the cells (struct field set → SHACL property paths → IRI constants → parser branches → emitter branches → test fixtures) is mechanical, the divergence modes are mechanical, the audit can be mechanical.

This feature_spec authors the TaskType declaration only — it does not implement any code. Implementation lands when a downstream feature_spec carries `task_type: add-artifact-type` in its front-matter and the FT-139 cluster dispatcher executes this cluster against it.

## Functional Specification

### Inputs

- The TaskType + Cell substrate from FT-139 (`crates/decision-cli/src/core/task_type/`) — this TaskType plugs into that registry.
- The classifier branch from FT-139 — recognises `task_type: add-artifact-type` on a consuming feature's front-matter.
- The capability resolver from FT-067/068 — each cell binds its model via this path.
- Reference implementations of the witnessed pattern (used as source of truth for cluster shapes and audit contract): the five shipped artifact-type features listed above. Pre-existing files this cluster's output must look like:
  - `crates/decision-cli/src/core/ontology/feedback.rs`, `capability.rs`, `verification_bench.rs`, `worker_image.rs` (Rust struct exemplars).
  - `crates/decision-cli/src/core/ontology/shapes/*.shacl.ttl` (SHACL shape exemplars).
  - `crates/decision-cli/src/core/vocab/*.rs` (IRI constant exemplars).
  - The parser + emitter + tests modules sitting next to the structs (exemplars for the remaining three cells).

### Outputs

**TaskType declaration:**

- `.product/features/FT-141-decision-cli-tasktype-add-artifact-type-...md` (this file) — the TaskType body. Per the FT-139 convention (`FT-TT-<name>` naming planned for future renames; for now FT-141 plays the role).
- Recognition signature: `task_type: add-artifact-type` in the consuming feature's front-matter.
- Cell cluster (6 cells, declared with `derived_from` order; the cluster's topological order produced by `Cluster::topo_order` is:
  1. `rust_struct`
  2. `shacl_shape`, `iri_module_consts` (both depend only on `rust_struct`)
  3. `parser`, `emitter` (both depend on `rust_struct` + `iri_module_consts`)
  4. `round_trip_tests` (depends on everything)

**Cell cluster declaration (the 6 cells):**

1. **`rust_struct`** — role: code-specialist.
   - Emits: `crates/decision-cli/src/core/ontology/<artifact_name>.rs`.
   - Body: `pub struct <Artifact> { pub field1: Type1, pub field2: Type2, ... }` with `#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]`.
   - `derived_from: []` — this is the root cell.
   - Model binding: `openai/code-small` (mechanical struct shape; small specialist sufficient).
2. **`shacl_shape`** — role: code-specialist.
   - Emits: `crates/decision-cli/src/core/ontology/shapes/<artifact>.shacl.ttl`.
   - Body: declares `dec:<Artifact>Shape sh:targetClass dec:<Artifact>` with one `sh:property` block per field on the Rust struct, with appropriate `sh:path` + `sh:datatype` / `sh:nodeKind` + `sh:minCount` / `sh:maxCount`.
   - `derived_from: [rust_struct]` — reads the struct's field set as input.
   - Model binding: `openai/code-small`.
3. **`iri_module_consts`** — role: mechanical (no LLM; deterministic template).
   - Emits: `crates/decision-cli/src/core/vocab/<artifact>.rs`.
   - Body: the artifact's class IRI as a `NamedNode` constant + one predicate IRI constant per struct field (e.g. `pub const FOO_FIELD1: NamedNode<&str> = ...`).
   - `derived_from: [rust_struct]` — generated mechanically from the field set.
   - Model binding: deterministic template (no model dispatch — the cell renders a Jinja/handlebars template against the struct's field set; emitted by the cluster dispatcher without an LLM round-trip).
4. **`parser`** — role: code-specialist.
   - Emits: `crates/decision-cli/src/core/ontology/<artifact>/parser.rs`.
   - Body: function reading a quad iterator (or oxigraph store, matching the existing exemplars) into the Rust struct. Must handle every field — missing-field handling per `sh:minCount` from the SHACL shape (required vs optional).
   - `derived_from: [rust_struct, iri_module_consts]` — needs the struct shape and the IRI constants.
   - Model binding: `openai/code-small`.
5. **`emitter`** — role: code-specialist.
   - Emits: `crates/decision-cli/src/core/ontology/<artifact>/emitter.rs`.
   - Body: function producing `Vec<Quad>` from the Rust struct. Must emit every field, using the IRI constants from `iri_module_consts`.
   - `derived_from: [rust_struct, iri_module_consts]` — symmetric with parser.
   - Model binding: `openai/code-small`.
6. **`round_trip_tests`** — role: code-specialist.
   - Emits: `crates/decision-cli/src/core/ontology/<artifact>/tests.rs`.
   - Body: at minimum (a) a positive property-test-style test — construct an instance, emit, parse, assert structural equality; (b) a negative SHACL test — construct a malformed instance (missing a required field or wrong datatype), run the SHACL validator against the shape, assert the validator rejects it.
   - `derived_from: [rust_struct, shacl_shape, parser, emitter]` — needs everything.
   - Model binding: `openai/code-small`.

**Coherence audit:**

- `scripts/checks/cluster-audit-add-artifact-type.py` — the load-bearing audit per ADR-080 §Decision §3. Six checks, all mechanical:
  1. **Field coverage in SHACL shape.** Every `pub` field name on the emitted Rust struct must appear as an `sh:path` value in the emitted SHACL shape. Catches: struct gains a field, SHACL forgotten (or vice versa) — the most witnessed drift mode in the reference features.
  2. **IRI const reachability.** Every IRI constant declared in `iri_module_consts.rs` must be referenced by either `parser.rs` or `emitter.rs` at least once. Catches: predicate IRI declared but parser/emitter still hardcodes a stringly-typed IRI somewhere.
  3. **Parser field coverage.** Each struct field name must appear on the LHS of an assignment in `parser.rs` (regex over the file). Catches: parser silently drops a field.
  4. **Emitter field coverage.** Each struct field name must appear on the RHS of a quad-construction in `emitter.rs` (regex over the file). Catches: emitter silently drops a field.
  5. **Round-trip tests have both cases.** `tests.rs` must contain at least one positive case (round-trip equality) AND at least one negative SHACL case (malformed instance rejected by SHACL validator). Detected by function-name / assertion-shape regex. Catches: tests assert only the happy path.
  6. **Zero Python files in cluster output.** Cell-output paths must NOT include any `.py` file. Catches: misclassification with worker task types (`add-judge-worker`, `add-author-worker`) — both of those emit Python; this one emits only Rust + Turtle. If a `.py` file shows up in the outputs, the wrong TaskType was dispatched.

The audit runs once after all cells emit, asserts all six checks, BLOCKS commit if any check fails (exit 1) or surfaces `ClusterAuditUnrunnable` on exit 2 (missing input). Exit 0 = pass.

### State

- New on-disk (task type declaration): this feature_spec body.
- New on-disk (audit script): `scripts/checks/cluster-audit-add-artifact-type.py`.
- New in Rust registry: a `TaskTypeDecl` entry named `add-artifact-type` populated alongside FT-139's `add-judge-worker` entry. Prompt templates live under `crates/decision-cli/src/core/task_type/templates/add_artifact_type/` — one per cell (6 templates, plus the `iri_module_consts` template that is rendered deterministically without LLM dispatch).
- Preserved: every existing artifact-type module already on disk (`feedback.rs`, `capability.rs`, `verification_bench.rs`, `worker_image.rs`, etc.) — none migrated through this cluster retroactively. This cluster runs on NEW artifact types only.
- No orchestration-store schema change; no on-disk artifact schema change.

### Behaviour

#### Phase 1 — Declare the cluster (TaskType + Cells)

1. Register `add-artifact-type` in the static TaskType registry from FT-139 with the 6-cell `TaskTypeDecl` declared above.
2. Each `CellDecl` populated with: `name`, `artifact_type` (the produced file's logical type — `rust-source`, `shacl-shape`, `iri-vocab-source`, `rust-source`, `rust-source`, `rust-test-source`), `prompt_template_path` under `crates/decision-cli/src/core/task_type/templates/add_artifact_type/`, `model_binding_capability_iri` (`openai/code-small` for five of six; deterministic template marker for `iri_module_consts`), `derived_from` per the table above.
3. `Cluster::topo_order` (from FT-139) returns a valid + deterministic ordering: `rust_struct` first; `shacl_shape` and `iri_module_consts` in stable order after; `parser` and `emitter` in stable order after that; `round_trip_tests` last. The deterministic order is asserted by TC-A.

#### Phase 2 — Implement the coherence audit script

1. `scripts/checks/cluster-audit-add-artifact-type.py` implements the 6 checks listed under §Outputs.
2. Script receives cell-output paths as CLI args (one per emitted cell — 6 paths). Self-discovers which arg is which by file extension + path convention.
3. Exit 0 = pass; exit 1 = audit failure with stderr describing which check failed and the offending field / IRI / file; exit 2 = unrunnable (missing input file, parse error). Standard Python only — no decision-cli dep — so it runs in any worker environment.

#### Phase 3 — Register the TaskType in the Rust registry

1. Extend `crates/decision-cli/src/core/task_type/registry.rs` (the registry introduced by FT-139) with the `add-artifact-type` entry.
2. The classifier branch from FT-139 already routes any feature_spec with `task_type: add-artifact-type` to `Action::DispatchCluster { task_type_name: "add-artifact-type" }` once this entry is registered — no classifier code change.

#### Phase 4 — Tests

1. **TC-A (exit-criteria)** — `Cluster::topo_order` returns a valid, deterministic order for `add-artifact-type`'s 6 cells. Asserts: ordering respects every `derived_from` edge; the order is stable across invocations (tie-breaking is deterministic); `rust_struct` is first, `round_trip_tests` is last.
2. **TC-B (scenario, positive)** — Coherence audit passes on a positive fixture. Fixture: a synthetic 6-file output where the Rust struct has fields `name`, `domain`, `payload`; the SHACL shape declares `sh:path` for each; the IRI module declares one const per field, all referenced by parser + emitter; parser assigns each field; emitter writes each field; tests have both a round-trip positive and a SHACL-rejects-malformed negative. Audit exits 0.
3. **TC-C (scenario, negative — SHACL field coverage)** — Coherence audit FAILS when the SHACL shape omits one field present in the Rust struct. Fixture: same as TC-B but the SHACL shape drops the `sh:property` block for `payload`. Audit exits 1 with stderr naming `payload` as the missing field and `shacl-field-coverage` as the failing check.
4. **TC-D (scenario, no-python)** — Coherence audit FAILS (or skips with exit 2) when the fixture contains a `.py` file in the cell outputs. Catches misclassification with `add-judge-worker` / `add-author-worker`. Fixture: TC-B's positive fixture plus a stray `agent_loop.py`. Audit exits non-zero with stderr naming `no-python-files` as the failing check.

#### Phase 5 — (intentionally elided)

This TaskType declaration is authored only; no Phase 5 implementation slice. Real implementation runs when a consuming feature_spec carries `task_type: add-artifact-type` and the FT-139 dispatcher executes this cluster.

### Invariants

- **Six cells, ordered.** The cluster's `derived_from` DAG is the contract; the dispatcher honours it via `Cluster::topo_order`. Adding a seventh cell or reordering existing edges is a TaskType amendment, not a silent change.
- **Audit teeth across the contract surface.** Field set ↔ SHACL `sh:path` set ↔ parser LHS set ↔ emitter RHS set must be the same set on the way through. The 6 audit checks codify this. A new audit check is the right response to a witnessed drift mode; loosening an existing check is an amendment with a recorded reason.
- **Zero Python in outputs.** Audit check 6 is non-negotiable — it is the misclassification firewall against the worker TaskTypes. If a downstream slice extends the cluster to need a Python helper, that helper belongs in a separate cluster.
- **Per-cell model binding.** Five cells bind to `openai/code-small`; `iri_module_consts` binds to a deterministic template (no LLM). Hardcoded model IDs in the dispatcher are a violation; the resolver path from FT-067/068 holds.
- **No retroactive migration.** The cluster runs on NEW artifact types only. The shipped artifact-type modules (FT-026/035/054/071/086) stay as they are; retrofitting them into the cluster pattern is out of scope.

### Error handling

- **Cell emit fails (LLM error, file write error)** → cluster aborts at that cell, prior cells rolled back via worktree reset, emit `ClusterCellFailed { cell }` (the FT-139 outcome type).
- **Coherence audit exits 1** → `ClusterAuditFailed { check, detail }` with `check` set to one of `{shacl-field-coverage, iri-const-reachability, parser-field-coverage, emitter-field-coverage, round-trip-tests-both-cases, no-python-files}`. No commit lands; worktree rolled back.
- **Coherence audit exits 2** → `ClusterAuditUnrunnable { stderr }`. Distinct outcome (operator-actionable) — the audit harness needs fixing, not the cells.
- **Capability resolver returns no binding for `openai/code-small`** → `ClusterDispatchError::NoCapabilityForCell { cell, role: "code-specialist" }` before any cell emits.
- **`iri_module_consts` deterministic template render fails** → `ClusterCellFailed { cell: "iri_module_consts" }` with the template engine error in `detail`.

### Boundaries

- **In scope.** Authoring this TaskType declaration; the 6-cell cluster shape with `derived_from` edges; per-cell model bindings; the coherence audit script's 6 checks; 4 TCs (1 exit-criteria + 3 scenarios) covering topo-order, positive audit, negative-shacl-coverage audit, and no-python audit.
- **Out of scope.** Implementing the audit script (the script is declared here; the implementation lands when a downstream slice carries the feature spec referencing this TaskType, or as a follow-on slice). Implementing the prompt templates for the 5 LLM-bound cells. Implementing the deterministic template for `iri_module_consts`. Running the cluster end-to-end against a real new artifact type (lands when an actual `add-artifact-type` consumer feature arrives). Mixed-feature composition with other TaskTypes in the same feature (FT-139 §Out of scope already defers this). Retrofitting shipped artifact-type features (FT-026/035/054/071/086) through the cluster. Promotion of TaskType + Cell to first-class product-cli artifact types — itself a future `add-artifact-type` cluster invocation, closing the bootstrap loop per ADR-080's rejected alternative §3.

## Out of scope

- Implementation of the 6 cell prompt templates and the deterministic `iri_module_consts` template.
- Implementation of `scripts/checks/cluster-audit-add-artifact-type.py` (declared here; lands as a follow-on).
- End-to-end execution against a real new artifact-type consumer feature.
- Retrofitting shipped artifact-type modules (FT-026/035/054/071/086) into the cluster pattern.
- Promotion of TaskType + Cell to first-class product-cli artifact types.
- Mixed-feature composition (the consuming feature carrying `task_type: add-artifact-type` PLUS another TaskType).
- LLM-based classifier or embedding similarity for matching feature → TaskType.
- Per-cell retry / partial-cluster resume semantics beyond FT-139's existing rollback path.