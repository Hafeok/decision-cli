---
id: FT-139
title: 'decision-cli: TaskType + Cell catalog substrate and first task type — add-judge-worker — with coherence audit prototype'
phase: 5
status: planned
depends-on: []
adrs:
- ADR-080
- ADR-072
tests:
- TC-370
- TC-371
- TC-372
- TC-373
- TC-374
domains:
- api
domains-acknowledged:
  observability: FT-139 ships 5 TCs (TC-370 exit-criteria + TC-371/372/373/374 scenarios) satisfying ADR-072. ADR-072 spans api + observability. Observability concerns are covered by TC-373 (integration test asserts cluster_dispatch's end-to-end behaviour with cell-by-cell emit + audit + finalize commit observable) and TC-372 (negative audit case asserts audit failure surfaces with a specific check identifier — the audit's "teeth" property observable). Explicit acknowledgement per ADR-072 review gate.
---

## Description

The implementation slice for [ADR-080](ADR-080)'s decision to apply DDD's task/cell decomposition to decision-cli's self-implementation pipeline. Ships three artefacts together because they are inseparable: (a) the TaskType + Cell vocabulary substrate, (b) the first TaskType — `add-judge-worker` — as the load-bearing prototype, (c) the coherence audit for that cluster, on which the whole pattern's safety property is validated.

Witnessed motivating gap: FT-126 + FT-127 shipped identical Python judge-worker boilerplate (pyproject.toml, agent loop, Pydantic IO, system prompt, capability binding, role catalog seed, unit tests) through the broad code-writer — 80% the same diff. FT-128, FT-129, FT-130, FT-132, FT-133 are queued to do it again. Routing each through a one-shot broad-worker dispatch is unaudited and unnecessary.

One subcommand → one slice — this slice extends `dec drive ship`'s planner registry with a classifier branch and a cluster-dispatch executor; it does not introduce a new top-level verb.

## Functional Specification

### Inputs

- The current `dec drive ship` planner stack (`features/drive/run.rs`, `features/drive/inspect_dor.rs`, `features/ft_119_drive_def_ready/planner.rs`, `features/drive/planners/feature_ship.rs`).
- The post-FT-123 in-process LiteLLM agentic loop in `workers/code-writer/src/code_writer/agent/loop.py` — this is the broad-worker fallback the classifier escapes to.
- Two reference implementations of the witnessed pattern: `workers/tc-author/` (FT-126) and `workers/tc-quality/` (FT-127) — used as the source of truth for the cluster's cell shapes and the coherence audit's expected contract surface.
- The capability resolver from FT-067/068 — each cell's model binding plugs in here.

### Outputs

**Substrate:**

- `crates/decision-cli/src/core/task_type/` (new module): `TaskTypeDecl`, `CellDecl`, `Cluster`, `CoherenceAudit` Rust types. No SHACL shapes yet (substrate lives in feature_spec bodies, per ADR-080 §Decision §1).
- `crates/decision-cli/src/features/drive/cluster_dispatch.rs` (new): cluster executor that walks Cells in `derived_from` order, runs the coherence audit at the end, and emits a `ClusterOutcome` for the run history.
- Classifier extension in `crates/decision-cli/src/features/drive/planners/feature_ship.rs`: a new precedence-ordered branch that reads `task_type:` from the feature_spec front-matter (high-confidence path) and dispatches the matched TaskType's cluster, or falls through to the broad-worker action (existing path) on absent / unknown values.

**First TaskType — `add-judge-worker`:**

- `FT-TT-add-judge-worker.md` under `.product/features/` (using the proposed `FT-TT-<name>` convention from ADR-080 — not yet a first-class artifact type) declaring:
  - Recognition signature: `task_type: add-judge-worker` in front-matter.
  - Cell cluster (with `derived_from` order):
    1. **`capability_binding`** — emits the role + capability + binding seed quads (`workers/<name>/seed.nq`-style). Model: small/mechanical; possibly no LLM (deterministic generation from a template).
    2. **`pydantic_io_models`** — emits `workers/<name>/src/<name>/models.py` with input + output Pydantic types. Derived from: spec body's "Inputs"/"Outputs" sections. Model: code-specialist (Anthropic-format or OpenAI-format depending on capability binding).
    3. **`system_prompt`** — emits `workers/<name>/src/<name>/prompts/system.md`. Derived from: spec body's "Behaviour"/"Acceptance criteria" + the pydantic_io_models contract. Model: stronger reasoning (the prompt is where the judge's actual semantics live).
    4. **`agent_loop`** — emits `workers/<name>/src/<name>/agent/loop.py` (or judge-specialised equivalent). Derived from: all of the above + the LiteLLM call shape. Model: code-specialist.
    5. **`unit_tests`** — emits `workers/<name>/tests/test_<name>.py` with fixture payloads that match the pydantic_io_models. Derived from: the pydantic_io_models + system_prompt's expected verdicts. Model: code-specialist.
  - Coherence audit: see "Coherence audit" below.

**Coherence audit for `add-judge-worker`:**

The audit holds the cluster together. It runs once after all cells emit, asserts the four-way agreement across the artefacts, and BLOCKS commit if any check fails:

1. **agent_loop calls LiteLLM with `model=payload.model_id` and `base_url=LITELLM_BASE_URL`** — assertion: the call site in the emitted `loop.py` matches the canonical FT-123 shape (regex over the file).
2. **capability_binding's `endpoint` + `model_id` are valid LiteLLM model strings** — assertion: model_id starts with a recognised provider prefix (`anthropic/`, `openai/`, `scaleway/`, `claude/`); endpoint is a valid URL or `default`.
3. **pydantic_io_models' input schema matches what agent_loop reads** — assertion: every `payload.<field>` accessed in `loop.py` exists as a field on the input model class.
4. **unit_tests' fixture payload validates against the input pydantic model** — assertion: the test loads the input model and passes; absence of a fixture test that constructs the input payload is itself a failure.
5. **system_prompt references field names that exist on the input model** — assertion: every Jinja/template variable in `system.md` is either a literal field name on the input model or appears in a known set of template helpers.

The audit is implemented as a single Python script invoked by the cluster_dispatch executor after cells emit. Fail-loud: any check failing aborts the cluster, rolls back the worktree write, and surfaces a `ClusterAuditFailed { check, detail }` outcome.

**Classifier + dispatcher:**

- `feature_ship::classify_for_task_type(feature_id) -> Option<TaskTypeMatch>` — reads the front-matter `task_type:` field via product_core, returns `Some(TaskTypeMatch { name, confidence: High })` if the value names a registered TaskType, `None` otherwise.
- `FeatureShipPlanner` gains a precedence branch: after the existing TC/VG checks, BEFORE the implementer dispatch action, call `classify_for_task_type`. If `Some`, return `Action::DispatchCluster { task_type_name }`. If `None`, fall through to the existing `Action::DispatchImplementer` (the broad-worker path).
- `cluster_dispatch::run(workdir, ctx, args, task_type_name)` — looks up the TaskType, walks its Cells in `derived_from` order, runs each cell as its own session (with its own model binding per the capability resolver), runs the coherence audit, commits the worktree if all cells + audit pass.

### State

- Updated on-disk (substrate): new `crates/decision-cli/src/core/task_type/` module; new `crates/decision-cli/src/features/drive/cluster_dispatch.rs`; classifier branch in `features/drive/planners/feature_ship.rs`.
- New on-disk (task type): `.product/features/FT-TT-add-judge-worker.md` declaring the cluster.
- New on-disk (audit): `scripts/checks/cluster-audit-add-judge-worker.py`.
- Preserved on-disk: every existing planner action; the broad-worker dispatch path; the existing role catalog; verify-graph-author's capability binding pattern.
- No orchestration-store schema change; no on-disk artifact schema change.

### Behaviour

#### Phase 1 — TaskType + Cell substrate (Rust core)

1. New module `crates/decision-cli/src/core/task_type/`. Types:
   - `TaskTypeDecl { name: String, cells: Vec<CellDecl>, coherence_audit: CoherenceAuditSpec }`.
   - `CellDecl { name: String, artifact_type: String, prompt_template_path: PathBuf, model_binding_capability_iri: NamedNode, derived_from: Vec<String> }`.
   - `CoherenceAuditSpec { script_path: PathBuf, timeout_seconds: u32 }`.
   - `Cluster::topo_order(cells: &[CellDecl]) -> Result<Vec<String>, PlanError>` — Kahn-style topo sort over `derived_from`; cycle is a PlanError.
2. Static TaskType registry — a `lazy_static` map from name → `TaskTypeDecl`, populated at startup. The first entry is `add-judge-worker` (see Phase 3 below).
3. Unit tests: topo order correctness, cycle detection, missing-derived-from-target detection.

#### Phase 2 — Classifier branch + cluster dispatcher

1. In `features/drive/planners/feature_ship.rs`, add `classify_for_task_type` helper that reads the feature's front-matter via the existing product_core path. Returns `Option<String>` (the TaskType name).
2. Extend the classifier table with a row positioned **between** the existing TC/VG checks and the implementer dispatch action:
   ```
   ... existing TC + VG checks ...
   feature_spec carries task_type: <name> matching a registered TaskType   → DispatchCluster { task_type_name }
   ... existing implementer dispatch branch (the broad-worker fallback) ...
   ```
3. Add `Action::DispatchCluster { task_type_name: String, feature_id: String }` to the `Action` enum.
4. Implement `features/drive/cluster_dispatch::run(workdir, ctx, args, task_type_name)`:
   - Look up the TaskType in the registry.
   - For each cell in `Cluster::topo_order(cells)?`:
     - Resolve the model binding via the capability resolver (same path verify-graph-author uses).
     - Render the prompt template against the upstream cells' bundles.
     - Dispatch a per-cell session (one LiteLLM call per cell; cell name + cluster id in the session record's IRI).
     - Persist the emitted artifact into the worktree at the path declared by the cell.
   - Once every cell has emitted, run the coherence audit (Phase 4).
   - If audit passes: stage everything, run finalize (commit + status flip) via the existing path.
   - If audit fails: roll back the worktree edits, emit a `ClusterAuditFailed` outcome, do NOT commit.

#### Phase 3 — Declare `add-judge-worker` TaskType

1. Create `.product/features/FT-TT-add-judge-worker.md` with front-matter `id: FT-TT-add-judge-worker`, `kind: task-type` (informational; not enforced by product-cli yet), `phase: 5`, and a body declaring:
   - Recognition signature: `task_type: add-judge-worker` in the consuming feature's front-matter.
   - Cells (the five listed in §Outputs above), each with name + artifact_type + prompt_template_path + model_binding_capability_iri + derived_from.
   - Coherence audit: pointer to `scripts/checks/cluster-audit-add-judge-worker.py`.
2. Populate the Rust registry with the corresponding `TaskTypeDecl`. Prompt templates live under `crates/decision-cli/src/core/task_type/templates/add_judge_worker/` (one Jinja-style file per cell).

#### Phase 4 — Coherence audit script

1. `scripts/checks/cluster-audit-add-judge-worker.py` implements the 5 checks listed under §Outputs.
2. Script receives the cell-output paths as CLI args (one path per emitted cell).
3. Exit 0 = pass; exit 1 = audit failure with stderr describing which check failed and why; exit 2 = unrunnable (missing input).
4. Uses standard Python (no decision-cli dep) so it can run in any worker environment.

#### Phase 5 — Tests

1. **TC-370 (exit-criteria)** — Topo order: `Cluster::topo_order` returns a valid, deterministic topological order for `add-judge-worker`'s 5 cells; cycle detection rejects malformed `derived_from` graphs.
2. **TC-371** — Classifier branch: a fixture feature_spec with `task_type: add-judge-worker` in front-matter produces `Action::DispatchCluster`; absent/unknown `task_type` falls through to `Action::DispatchImplementer` (the broad-worker escape hatch).
3. **TC-372** — Coherence audit teeth (negative): the audit script catches an `agent_loop.py` field reference absent from `pydantic_io_models.py`, surfacing a specific check identifier. The load-bearing test that proves the cluster's safety property holds.
4. **TC-373** — Integration: a tempdir feature with `task_type: add-judge-worker` runs through `cluster_dispatch::run`, emits all 5 cells, the audit passes, and a `[FT-T373]` commit lands. Mocks LiteLLM at the cell-dispatch boundary.
5. **TC-374** — Coherence audit teeth (positive): the audit script accepts a synthetic fixture where all 5 cells agree on the input contract. Pair with TC-372 — together they prove the audit discriminates (negative fails, positive passes).

### Invariants

- **Broad-worker fallback is non-optional.** Removing or breaking the existing implementer dispatch path is a TC-violating change. The classifier branch is purely additive at higher precedence.
- **Cluster atomicity.** Either all cells emit + audit passes + finalize commits, or the worktree is rolled back and no commit lands. No partial-cluster artifacts in git history.
- **Audit failure is loud.** A `ClusterAuditFailed` outcome surfaces in drive history with the failing check identifier verbatim — the operator can map it back to the audit script's `check` enum without grepping.
- **Per-cell model binding.** Each cell resolves its own capability binding; no hardcoded model_ids or endpoints in the cluster dispatcher. This is the FT-067/068 pattern, replicated.
- **TaskType registry is static at startup.** No graph-resident TaskType lookup yet; the registry's contents are baked into the binary. Promotion to graph-resident TaskType artifact is a later slice.

### Error handling

- **Cycle in `derived_from`** → `PlanError::ClusterCycle { cycle_path }` at TaskType registration time (caught at startup, not at first dispatch).
- **Unknown TaskType in `task_type:` field** → classifier returns `None` and falls through to the broad worker (treated as low-confidence — consistent with ADR-080's escape-hatch principle).
- **Capability resolver returns no binding for a cell** → `ClusterDispatchError::NoCapabilityForCell { cell, role }`; cluster aborts before any cell emits.
- **Cell emit fails (LiteLLM error, file write error)** → cluster aborts at that cell; cells already emitted are rolled back via worktree reset; emit a `ClusterCellFailed` outcome with the cell name.
- **Audit script returns exit 2 (unrunnable)** → cluster outcome is `ClusterAuditUnrunnable`; same rollback semantics as audit-failed but a distinct outcome so the operator can fix the audit harness.

### Boundaries

- **In scope.** Five phases above; substrate Rust types + dispatcher; classifier branch; `add-judge-worker` TaskType declaration + 5-cell cluster + audit script; 5 TCs.
- **Out of scope.** Other TaskTypes (`add-author-worker`, `add-artifact-type`, etc.) — drafted in parallel as separate feature_specs, not implemented here. Promotion of TaskType + Cell to first-class product-cli artifact types (a later "add-an-artifact-type" cluster will land that). Embedding-similarity or LLM-based classification — v1 uses operator-declared `task_type:` only. Mixed-feature composition (multiple TaskTypes in one feature) — v1 dispatches the first matched TaskType; multi-TaskType composition is a future slice. UI for visualising cluster outcomes in `dec drive show` — a follow-on. Retrofitting verify-graph-author into the cluster pattern — it already has the capability resolver path; conversion is a later cleanup.

## Out of scope

- Other TaskType clusters (drafted in parallel under separate FT-TT-* slices).
- TaskType + Cell as first-class product-cli artifact types.
- LLM-based classifier or embedding similarity.
- Mixed-feature composition (multi-TaskType per feature).
- UI / `dec drive show` rendering of cluster outcomes.
- Backfilling existing workers (tc-author, tc-quality) into the cluster pattern.
- Cell-level retry / partial-cluster resume.
- Per-cell secrets management beyond the capability resolver's existing path.
