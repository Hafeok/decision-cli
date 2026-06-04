---
id: FT-144
title: 'decision-cli: TaskType extend-role-catalog-seed — add a role/capability/authority/predicate to the seeded role catalog with seed function + wiring + SHACL + tests'
phase: 5
status: planned
depends-on: []
adrs:
- ADR-080
- ADR-072
tests:
- TC-366
- TC-367
- TC-368
- TC-369
domains:
- api
- security
domains-acknowledged:
  observability: FT-144 ships 4 TCs (TC-A exit-criteria + TC-B/C/D scenarios) satisfying ADR-072. ADR-072 spans api + security + observability per the cross-cutting domain rules; the observability obligation is satisfied by the audit's structural checks themselves and by TC-D's explicit lock-in. The audit script (scripts/checks/cluster-audit-extend-role-catalog-seed.py) emits a numbered check identifier (1..6) on failure, surfaced verbatim in the ClusterAuditFailed outcome — the cluster's safety property becomes observable rather than implicit. The load-bearing fail-closed guarantee from ADR-069 (a legacy store without the new predicate must lookup to a safe default, never panic) is itself observable through audit check 5, which inspects the round_trip_tests cell output for a test whose function name matches legacy_store_lookup_returns_safe_default; TC-D drives an end-to-end run with that test deliberately removed and asserts the audit fails with check identifier 5. The observability concern — "can the operator see why the cluster failed and which guarantee was violated" — is therefore covered by the audit-check-identifier surfacing pattern (audit teeth are visible) and by TC-D specifically locking in that the fail-closed test's presence is enforced structurally, not by hope.
---

## Description

The TaskType declaration for `extend-role-catalog-seed`: features that add a new role, capability binding, authority declaration, or predicate to the role catalog seeded into the orchestration store. This is the second TaskType authored against the substrate shipped by [FT-139](FT-139) (which built the TaskType + Cell vocabulary and the `add-judge-worker` prototype) under the regime declared by [ADR-080](ADR-080) (DDD task/cell decomposition).

Witnessed motivating examples — three diffs that share an identical pattern:

- **FT-068** (shipped): verify-graph-author capability binding. Added the `endpoint` + `model_id` IRIs to seeds.rs, a `verify_graph_author_capability_seed_quads()` function, wiring into `seed_role_catalog()`, and round-trip tests asserting the binding resolves through the capability resolver.
- **FT-121** (just shipped): `dec:roleTool` predicate seeded on implementer + verifier roles. Added the `ROLE_TOOL_IRI` constant, a `role_tool_seed_quads()` function, wiring into `seed_role_catalog()`, the SHACL shape extension for `dec:roleTool` cardinality, the `allowed_tools: Vec<String>` field on `Role`, and round-trip tests including the load-bearing `legacy_store_lookup_returns_safe_default` assertion (per ADR-069's fail-closed contract — a pre-FT-121 store without the predicate must lookup-to-empty-Vec, not panic).
- **FT-066 follow-on** (deferred IOU — see `lifecycle.rs:46-49` comment): plug code-writer through the capability resolver. Same pattern: an IRI constant for the code-writer capability, a seed-quad function, wiring into `seed_role_catalog()`, and a round-trip test asserting `lookup("code-writer").endpoint` resolves.

All three diffs cluster into the same shape — IRI constants, a seed function, a wiring line, optional SHACL extension if a new predicate has cardinality constraints, optional Rust struct field extension if the predicate surfaces on the `Role` API, and round-trip tests including a fail-closed default assertion. Routing each of these through the broad code-writer re-derives the boilerplate every time; the deferred FT-066 follow-on is precisely the kind of routine work that has rotted as an IOU because the broad worker treats it as a fresh problem.

This TaskType encodes the pattern as a cell cluster.

## Functional Specification

### Inputs

- The TaskType + Cell substrate from [FT-139](FT-139): `crates/decision-cli/src/core/task_type/` (TaskTypeDecl, CellDecl, Cluster, CoherenceAuditSpec), the static TaskType registry, `cluster_dispatch::run`, and the classifier branch on `task_type:` front-matter.
- The role catalog substrate: `crates/decision-cli/src/core/role_catalog/seeds.rs` (IRI constants + seed-quad functions), `crates/decision-cli/src/core/role_catalog/role.rs` (`pub struct Role` + `collect_*` helpers + `lookup()`), `crates/decision-cli/src/core/role_catalog/seeds/*.shacl.ttl` (SHACL shapes), `crates/decision-cli/src/core/role_catalog/tests.rs` (round-trip tests).
- The init wiring: `crates/decision-cli/src/features/init/pipeline.rs::seed_role_catalog()` — the function that composes all the `*_seed_quads()` outputs into the orchestration store's initial transaction.
- Witnessed diffs to mine for shape: the FT-068 commit, the FT-121 commit, and the deferred-FT-066 IOU comment at `lifecycle.rs:46-49`.
- Two cluster invocation parameters that this TaskType honours:
  - **`requires_shacl: bool`** — set true when the change introduces a new predicate with cardinality constraints that the SHACL shape must enforce (e.g. FT-121's `dec:roleTool` with `sh:minCount 0; sh:maxCount unbounded`). Set false for additive IRI seeds with no cardinality story (e.g. FT-068's capability binding which slots into an existing shape).
  - **`surfaces_on_role_struct: bool`** — set true when the predicate is read back via `Role::lookup()` and consumers expect it as a typed field on `pub struct Role` (e.g. FT-121's `allowed_tools: Vec<String>`). Set false when the predicate is store-internal (e.g. authority links walked by SPARQL queries, not surfaced in the Rust API).

### Outputs

**TaskType declaration:**

- `.product/features/FT-TT-extend-role-catalog-seed.md` (using the proposed `FT-TT-<name>` convention from ADR-080 §Decision §1 — not yet a first-class artifact type) declaring:
  - Recognition signature: `task_type: extend-role-catalog-seed` in the consuming feature's front-matter.
  - Cluster invocation parameters: `requires_shacl: bool`, `surfaces_on_role_struct: bool`.
  - Cell cluster (with `derived_from` ordering): the six cells listed below (4 mandatory + 2 conditional).
  - Coherence audit: pointer to `scripts/checks/cluster-audit-extend-role-catalog-seed.py`.

**Cluster — six cells in `derived_from` order:**

1. **`iri_constants`** (mechanical, mandatory)
   - Artifact type: Rust source insertion in `crates/decision-cli/src/core/role_catalog/seeds.rs`.
   - Emits `pub const <NAME>_IRI: &str = "https://decision-cli.dev/ns/...";` constants for each new role / authority / predicate IRI required by the consuming feature.
   - `derived_from: []`.
   - Model binding: deterministic template (no LLM — IRI names + values are mechanical projections from the consuming feature_spec's "Inputs"/"Outputs" sections).

2. **`seed_quad_function`** (code-specialist, mandatory)
   - Artifact type: Rust function in `crates/decision-cli/src/core/role_catalog/seeds.rs`.
   - Emits `pub fn <thing>_seed_quads() -> Vec<Quad>` producing every new triple: role-tool predicates, authority links, capability bindings, escalation hints, etc. References the IRI constants emitted by upstream cell 1.
   - `derived_from: [iri_constants]`.
   - Model binding: `openai/code-small` (mechanical-ish but enough variability in quad shape that a code-specialist is the right size).

3. **`init_pipeline_wiring`** (mechanical, mandatory)
   - Artifact type: Rust source insertion in `crates/decision-cli/src/features/init/pipeline.rs::seed_role_catalog()`.
   - Emits the one-line `.extend(<thing>_seed_quads())` append into the quads vec composed by `seed_role_catalog()`.
   - `derived_from: [seed_quad_function]`.
   - Model binding: deterministic template (one line of code with a known shape; LLM would be overkill).

4. **`shacl_shape_extension`** (code-specialist, **conditional on `requires_shacl: true`**)
   - Artifact type: Turtle insertion in `crates/decision-cli/src/core/role_catalog/seeds/<shape>.shacl.ttl`.
   - Extends the relevant SHACL shape with a `sh:property [ sh:path <new-predicate> ; sh:datatype/sh:class ... ; sh:minCount ... ; sh:maxCount ... ]` clause for each new predicate that requires cardinality enforcement. Skipped entirely when `requires_shacl: false`.
   - `derived_from: [iri_constants, seed_quad_function]`.
   - Model binding: `openai/code-small`.

5. **`role_struct_field_extension`** (code-specialist, **conditional on `surfaces_on_role_struct: true`**)
   - Artifact type: Rust source insertion in `crates/decision-cli/src/core/role_catalog/role.rs`.
   - Extends `pub struct Role` with a new `pub <field>: <Type>` field (e.g. `pub allowed_tools: Vec<String>` from FT-121). Also extends the corresponding `collect_<field>()` helper that walks the store for the new predicate, and threads the helper's output through `lookup()` so `Role::lookup(role_id).<field>` returns the seeded value. Skipped entirely when `surfaces_on_role_struct: false`.
   - `derived_from: [iri_constants]`.
   - Model binding: `openai/code-small`.

6. **`round_trip_tests`** (code-specialist, mandatory)
   - Artifact type: Rust tests in `crates/decision-cli/src/core/role_catalog/tests.rs`.
   - Emits at minimum four tests:
     - (a) **seed → lookup → assert**: seed_role_catalog runs, `Role::lookup(role_id)` returns a Role whose new field / predicate is populated with the expected value.
     - (b) **`legacy_store_lookup_returns_safe_default`**: an orchestration store snapshot from BEFORE this seed extension landed (or a synthetic store with the new quads stripped) — `Role::lookup(role_id)` returns the safe default (empty `Vec`, `None`, or zero-value per ADR-069's fail-closed contract). This is the load-bearing test that ADR-069 demands and FT-121 prototyped — it must not be omitted; the coherence audit asserts its presence by name.
     - (c) **SHACL validation passes**: only emitted if `requires_shacl: true`. Loads the seeded store, runs SHACL validation against the extended shape, asserts conformance.
     - (d) **SHACL validation FAILS on malformed instance**: only emitted if `requires_shacl: true`. Constructs a deliberately-malformed instance (wrong cardinality, wrong datatype) and asserts SHACL surfaces a violation. Lock-in test that the shape has teeth.
   - `derived_from: [seed_quad_function, init_pipeline_wiring, shacl_shape_extension (if requires_shacl), role_struct_field_extension (if surfaces_on_role_struct)]`.
   - Model binding: `openai/code-small`.

**Coherence audit:**

`scripts/checks/cluster-audit-extend-role-catalog-seed.py` runs once after all cells emit and asserts six checks. It is the load-bearing audit owned by this TaskType (per ADR-080's safety property: *"The coherence audit is the load-bearing audit of the whole pattern"*):

1. **Every IRI constant in `iri_constants` is referenced by `seed_quad_function`.** Static scan: parse `pub const <NAME>_IRI: &str = "...";` declarations from the iri_constants cell's output, then check the seed_quad_function output for at least one textual reference to `<NAME>_IRI` per constant. Unreferenced constants are dead seeds and fail the audit.
2. **`seed_quad_function` is wired into `seed_role_catalog()`.** Regex over the init_pipeline_wiring cell's output: the seed function's name must appear on the right-hand side of `.extend(` inside the `seed_role_catalog()` body. A missing wiring line is a silent dropped-cell failure.
3. **If `shacl_shape_extension` is present, every NEW predicate IRI it adds (parsed from `sh:path <iri>` clauses) has a matching seed quad emitted by `seed_quad_function`.** Cross-cell agreement: the SHACL shape's enforced predicates must actually be seeded; otherwise SHACL validation will flag every freshly-seeded store as nonconformant.
4. **If `role_struct_field_extension` is present, the new struct field's value type matches what `seed_quad_function` emits AND the `collect_<field>` helper is called from `lookup()`.** Type-shape contract: a `pub field: Vec<String>` field requires the seed function to emit string-literal objects; a `pub field: NamedNode` field requires IRI-typed objects. Mismatches surface here, not at runtime under a user's deserializer. The `collect_<field>` call from `lookup()` is the wiring check — without it the field is set to its default and the seeded value is silently dropped (the exact failure mode FT-121 prevented).
5. **`round_trip_tests` has at least one test whose function name matches `legacy_store_lookup_returns_safe_default`.** This is the explicit lock-in for ADR-069's fail-closed guarantee, surfaced as an audit check rather than a hope. The test's *content* may vary by predicate; its *presence* must not. Per ADR-069 / FT-121: a store missing the predicate must lookup to a safe default, never panic, never crash the caller. The audit enforces that the test enforces that.
6. **Distinct from `extend-planner-classifier`.** File-glob assertion: every emitted artifact path under the cluster must reside under `crates/decision-cli/src/core/role_catalog/`, `crates/decision-cli/src/features/init/`, or `scripts/checks/`. **None** may live under `crates/decision-cli/src/features/drive/planners/` or touch `planner.rs` / `inspect_dor.rs`. This is the structural divider between the two TaskTypes — a misclassified feature that should have routed through `extend-planner-classifier` instead would emit a planner.rs touch, and this audit catches it. Audit fails loud rather than silently producing a planner change that the wrong cluster owns.

The audit is fail-loud — any check failing aborts the cluster, rolls back the worktree edits, and surfaces a `ClusterAuditFailed { check, detail }` outcome per FT-139's `cluster_dispatch::run` contract.

### State

- Updated on-disk (TaskType declaration): `.product/features/FT-TT-extend-role-catalog-seed.md`.
- Updated on-disk (audit): `scripts/checks/cluster-audit-extend-role-catalog-seed.py`.
- Updated on-disk (registry): the new TaskType entry added to the static registry in `crates/decision-cli/src/core/task_type/registry.rs` (per FT-139's substrate convention).
- Preserved on-disk: every existing seed function, the existing SHACL shapes, the existing `Role` struct, the existing init pipeline wiring, the existing tests. This TaskType is purely additive at the registry level.
- No orchestration-store schema change beyond what individual consuming features may add via the cluster.

### Behaviour

#### Phase 1 — Declare `extend-role-catalog-seed` TaskType

1. Create `.product/features/FT-TT-extend-role-catalog-seed.md` with front-matter `id: FT-TT-extend-role-catalog-seed`, `kind: task-type` (informational; not yet enforced by product-cli), `phase: 5`. Body declares:
   - Recognition signature: `task_type: extend-role-catalog-seed` in the consuming feature's front-matter.
   - Cluster parameters: `requires_shacl: bool` and `surfaces_on_role_struct: bool`, both required.
   - Six cells (the four mandatory + two conditional listed in §Outputs), each with `name`, `artifact_type`, `prompt_template_path`, `model_binding_capability_iri`, and `derived_from`.
   - Coherence audit pointer: `scripts/checks/cluster-audit-extend-role-catalog-seed.py`.

2. Add the `TaskTypeDecl` entry to the static registry per FT-139's pattern. Prompt templates live under `crates/decision-cli/src/core/task_type/templates/extend_role_catalog_seed/` (one Jinja-style file per cell — six files when both conditional cells are present, four files for the minimum case).

3. The classifier (already extended by FT-139) picks up the new entry automatically — no classifier branch changes required here.

#### Phase 2 — Coherence audit script

1. `scripts/checks/cluster-audit-extend-role-catalog-seed.py` implements the six checks listed under §Outputs §"Coherence audit". Script receives the cell-output paths as CLI args (one path per emitted cell — four or six paths depending on the conditional cells) plus a JSON sidecar `--params '{"requires_shacl": ..., "surfaces_on_role_struct": ...}'` so it knows which conditional cells to expect.
2. Exit 0 = pass; exit 1 = audit failure with stderr describing which numbered check failed and which artefact violated it; exit 2 = unrunnable (missing input path).
3. Uses standard Python (no decision-cli dep) so it runs in any worker environment, matching FT-139's `cluster-audit-add-judge-worker.py` convention.

#### Phase 3 — Conditional-cell branching in `cluster_dispatch`

1. `cluster_dispatch::run` (shipped by FT-139) already walks the cells in `derived_from` order. This TaskType adds the convention that two cells declare a `condition_param` field on `CellDecl` referencing the cluster invocation parameters; the dispatcher skips a cell when its condition evaluates false.
2. Substrate impact: if `CellDecl` does not yet have a `condition_param` field, this slice extends it (small, additive). Coordinated with FT-139's substrate; if FT-139's `CellDecl` already supports conditional cells, no substrate change is required.

#### Phase 4 — Tests

Four TCs (one exit-criteria + three scenarios) listed below in §Tests. The exit-criteria TC asserts the topo order over all six cells (including conditionals when both flags are true) is acyclic and deterministic. The three scenario TCs assert the audit's positive case, its IRI-mismatch negative case, and its missing-fail-closed-test negative case — the last being the ADR-069 lock-in.

### Invariants

- **Fail-closed default is non-optional.** Per ADR-069 (and prototyped by FT-121), a store missing the seeded predicate must lookup to a safe default; the round_trip_tests cell must include the `legacy_store_lookup_returns_safe_default` test, and audit check 5 enforces its presence. Removing this guarantee is a TC-violating change.
- **Cluster atomicity (inherited from FT-139).** Either all cells emit + audit passes + finalize commits, or the worktree is rolled back and no commit lands.
- **Audit failure is loud (inherited from FT-139).** A `ClusterAuditFailed` outcome surfaces in drive history with the failing numbered check identifier (1..6) verbatim.
- **Structural divider from `extend-planner-classifier`.** Audit check 6 enforces this at the file-glob level: this cluster touches `seeds.rs` / `role_catalog/role.rs` / `init/pipeline.rs` / SHACL shapes / role_catalog tests; it does NOT touch `planner.rs` or `inspect_dor.rs`. Misclassified features that emit a planner.rs touch get a loud audit failure, not a silent merge.
- **Conditional cells are skipped, not stubbed.** When `requires_shacl: false`, the `shacl_shape_extension` cell is not invoked at all — no empty SHACL stub gets written, no LLM call gets made. Same for `surfaces_on_role_struct: false` and `role_struct_field_extension`.
- **TaskType registry is static at startup (inherited from FT-139).** No graph-resident TaskType lookup yet.

### Error handling

- **Cycle in `derived_from`** → `PlanError::ClusterCycle { cycle_path }` at TaskType registration time (caught at startup per FT-139's contract).
- **`requires_shacl: true` but no `sh:path` declarations parseable from the SHACL extension output** → `ClusterAuditFailed { check: 3, detail: "shacl_shape_extension produced no parseable sh:path clauses" }`.
- **`surfaces_on_role_struct: true` but no `pub <field>:` declaration parseable from the struct extension output** → `ClusterAuditFailed { check: 4, detail: "role_struct_field_extension produced no parseable struct field" }`.
- **Missing `legacy_store_lookup_returns_safe_default` test** → `ClusterAuditFailed { check: 5, detail: "round_trip_tests did not emit a test matching legacy_store_lookup_returns_safe_default" }`. This is the ADR-069 lock-in firing.
- **Emitted artifact path matches `planner.rs` or `inspect_dor.rs`** → `ClusterAuditFailed { check: 6, detail: "cluster touched planner surface; misclassified — consider task_type: extend-planner-classifier" }`. The audit message itself names the likely correct TaskType, helping the operator re-route.
- **Cell emit fails (LiteLLM error, file write error)** → cluster aborts at that cell (inherited from FT-139's `cluster_dispatch::run`); cells already emitted are rolled back.

### Boundaries

- **In scope.** Four phases above; the TaskType declaration; the six-cell cluster (4 mandatory + 2 conditional on declared parameters); the coherence audit script; the static registry entry; the four TCs.
- **Out of scope.** Implementing the FT-066 deferred follow-on for code-writer — this TaskType *enables* its routine dispatch, but the consuming feature is its own slice. The other witnessed TaskTypes (`extend-planner-classifier`, `add-author-worker`, `add-artifact-type`, `add-cli-subcommand`) — drafted in parallel under separate FT-TT-* slices. Promotion of TaskType + Cell to first-class product-cli artifact types (deferred per ADR-080). Multi-TaskType composition within a single consuming feature (v1 dispatches the first matched TaskType). LLM-based classifier confidence (v1 uses operator-declared `task_type:` only). Backfilling FT-068 / FT-121 retroactively into the cluster pattern — they have already shipped through the broad worker. Retro-typing of conditional cells beyond `requires_shacl` / `surfaces_on_role_struct` (further parameters can be added in follow-on slices if witnessed).

## Out of scope

- The actual FT-066 deferred follow-on (code-writer through capability resolver) — drafted as a consuming feature in a later slice; this TaskType makes it cheap to ship when it comes.
- Other TaskType clusters (`extend-planner-classifier`, `add-author-worker`, `add-artifact-type`, `add-cli-subcommand`, etc.).
- Promotion of TaskType + Cell to first-class product-cli artifact types.
- Multi-TaskType composition within a single consuming feature.
- LLM-based or embedding-similarity classifier — v1 uses operator-declared `task_type:` only.
- Backfilling FT-068 / FT-121 into the cluster pattern retroactively.
- Additional conditional parameters beyond `requires_shacl` / `surfaces_on_role_struct`.
- Cell-level retry / partial-cluster resume (FT-139 substrate concern).
- Per-cell secrets management beyond the capability resolver's existing path.