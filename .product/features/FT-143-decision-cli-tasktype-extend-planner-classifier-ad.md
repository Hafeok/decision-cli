---
id: FT-143
title: 'decision-cli: TaskType extend-planner-classifier — add a row to a def-ready/ship planner''s classifier table with trait method + production impl + state-hash update + 4 TCs'
phase: 5
status: planned
depends-on: []
adrs:
- ADR-080
- ADR-072
tests:
- TC-362
- TC-363
- TC-364
- TC-365
domains:
- api
domains-acknowledged:
  observability: 'FT-143 ships 4 TCs (TC-355 exit-criteria + TC-356/357/358 audit scenarios) satisfying ADR-072. ADR-072 spans api + observability; api is the primary domain because the artefact is a TaskType + cell-cluster declaration consumed by the FT-139 dispatcher''s API. Observability concerns are covered by TC-357 and TC-358 — both assert audit failure surfaces with a specific check identifier verbatim on stderr (the audit''s "teeth" property; the operator can map the failing check back without grepping). TC-356 covers the positive observable: audit script exits 0 on a fixture that fits between named adjacent rows, surfacing the structural-agreement property of the cluster. Explicit acknowledgement per ADR-072 review gate, mirroring FT-138''s and FT-139''s pattern.'
---

## Description

TaskType declaration for **`extend-planner-classifier`** — features that add a row to either def-ready's `FeatureReadyPlanner` (FT-119 family) or ship's `FeatureShipPlanner` classifier table. Authored as a feature_spec body per [ADR-080](ADR-080)'s `FT-TT-<name>` convention. [FT-139](FT-139) ships the substrate (TaskType + Cell vocabulary, registry, cluster dispatcher, coherence audit harness) and the first task type (`add-judge-worker`); this slice extends the catalog with the second witnessed task type.

Witnessed motivating examples (the cluster shape is derived from their diffs):

- **[FT-138](FT-138)** (just shipped) — added "open implementer feedback for the feature → `Done`" row to `FeatureReadyPlanner`, positioned above the `vgs_cover` checks. Six pieces: inspector trait method (`has_open_implementer_feedback_for_feature`), default impl (`Ok(false)`), production impl in `inspect_dor.rs` with SPARQL `ASK` over the orchestration store, classifier row in `planner.rs`, state-hash inclusion in `classify_and_hash` (TC-349 was the silent-regression guard for this), and 4 unit tests (precedence, positive, negative, state-hash).
- **[FT-131](FT-131)** (shipped) — readiness-orchestrator extension to `FeatureReadyPlanner`. Same six-piece shape.
- **[FT-119](FT-119)** (shipped) — the original def-ready planner; established the inspector-trait + classifier-table pattern that everything since extends.

Routing FT-138/FT-131-style features through the broad code-writer re-derives the identical 6-piece pattern from scratch each time. This TaskType captures the pattern as a 6-cell cluster with a coherence audit that catches the silent-regression failure mode FT-138's TC-349 was authored against.

This is a **TaskType declaration feature_spec** under ADR-080's convention — it ships no Rust code itself; the cluster of cells gets dispatched when a future feature carries `task_type: extend-planner-classifier` in its front-matter and `dec drive ship` classifies it.

## Functional Specification

### Inputs

- The cell-cluster substrate from [FT-139](FT-139): `crates/decision-cli/src/core/task_type/` (TaskTypeDecl, CellDecl, Cluster::topo_order, CoherenceAuditSpec), the static registry, and `features/drive/cluster_dispatch.rs`.
- The capability resolver from FT-067/068 — each cell's model binding resolves through it.
- The current planner stacks in `crates/decision-cli/src/features/ft_119_drive_def_ready/planner.rs` (FeatureReadyPlanner) and `crates/decision-cli/src/features/drive/planners/feature_ship.rs` (FeatureShipPlanner). One of these is the target per cluster invocation — declared by the consuming feature_spec's body.
- The current `GraphInspector` trait + `inspect.rs` default impl + `inspect_dor.rs` production wiring in `crates/decision-cli/src/features/drive/`.
- Two reference implementations of the witnessed pattern: [FT-138](FT-138) (canonical 6-piece example) and [FT-131](FT-131) — the source of truth for the cluster's cell shapes and the coherence audit's expected contract surface.

### Outputs

This feature_spec ships **declarative artefacts only** — it adds one TaskType to the `FT-139` registry and one audit script. No Rust slice code lands here.

**TaskType declaration (this feature_spec's body, plus a registry entry):**

- Recognition signature: `task_type: extend-planner-classifier` in the consuming feature's front-matter.
- Per-cell entry in the static `TaskTypeRegistry` (the `lazy_static` map FT-139 establishes), populated from the cell declarations below.
- Coherence audit pointer: `scripts/checks/cluster-audit-extend-planner-classifier.py`.

**Cell cluster (6 cells, `derived_from` order encoded by each cell's `derived_from` list — Kahn-style topo sort by `Cluster::topo_order`):**

1. **`inspector_trait_method`** (cell type: code-specialist)
   - Artifact type: rust-source-fragment.
   - Emits: a new method signature on the `GraphInspector` trait in `crates/decision-cli/src/features/drive/inspect.rs`, of the form `fn check_<thing>(&self, feature_id: &str) -> Result<<RetType>, InspectError>` where `<thing>` and `<RetType>` come from the consuming feature_spec's body (e.g. FT-138 produced `has_open_implementer_feedback_for_feature` returning `Result<bool, InspectError>`).
   - `derived_from: []`.
   - Model binding: `openai/code-small` (signature emission is small-surface; no reasoning needed beyond naming + return-type discipline).
   - Prompt template: `crates/decision-cli/src/core/task_type/templates/extend_planner_classifier/inspector_trait_method.j2`.

2. **`inspector_default_impl`** (cell type: mechanical / template-deterministic)
   - Artifact type: rust-source-fragment.
   - Emits: the trait's default implementation directly below the signature, returning the most permissive value (`Ok(false)` for `bool`, `Ok(default)` for typed signals). This is the "test-friendly defaults that say nothing's wrong" pattern FT-138 §Phase 1 §2 documents — existing test stubs implementing `GraphInspector` must compile unchanged.
   - `derived_from: ["inspector_trait_method"]`.
   - Model binding: **deterministic template** — no LLM call. The cell's "model" is a Jinja-style render of `Ok(<default>)` over the upstream cell's return type. Bound to the `dec:capability:none` mechanical-cell capability the FT-139 substrate establishes.
   - Prompt template: not applicable (deterministic); template path: `templates/extend_planner_classifier/inspector_default_impl.j2`.

3. **`inspector_production_impl`** (cell type: code-specialist)
   - Artifact type: rust-source-fragment.
   - Emits: the production override of the trait method in `crates/decision-cli/src/features/drive/inspect_dor.rs`, reading the live orchestration store via SPARQL `ASK` / `SELECT` (the FT-138 pattern: `pub(super) fn has_open_implementer_feedback(workdir, product_root, feature_id) -> Result<bool, InspectError>` resolving feature TCs via the existing `resolve_feature_tcs_short` helper then running an `ASK` against the named-graph store) OR a filesystem walk if the underlying signal is on-disk-only.
   - `derived_from: ["inspector_trait_method"]`.
   - Model binding: `openai/code-small`.
   - Prompt template: `templates/extend_planner_classifier/inspector_production_impl.j2`.

4. **`classifier_row`** (cell type: code-specialist)
   - Artifact type: rust-source-fragment.
   - Emits: the guarded return in `classify_and_hash`'s first-match-wins ladder in the appropriate planner (`features/ft_119_drive_def_ready/planner.rs` for `FeatureReadyPlanner` OR `features/drive/planners/feature_ship.rs` for `FeatureShipPlanner` — selected by the consuming feature_spec). The row calls `self.inspector.<method>(feature_id)?` and on `Ok(true)` returns the declared `Action::*` variant. **Position relative to existing rows MUST be explicit** — the cell output includes a positional comment of the form `// FT-XXX / ADR-YYY: above <row_above>, below <row_below>` naming the two adjacent existing rows. The classifier table docstring at the top of the module must also be updated with the new row at the same documented precedence.
   - `derived_from: ["inspector_trait_method", "inspector_production_impl"]`.
   - Model binding: `openai/code-small`.
   - Prompt template: `templates/extend_planner_classifier/classifier_row.j2`.

5. **`state_hash_update`** (cell type: code-specialist)
   - Artifact type: rust-source-fragment.
   - Emits: the change to `classify_and_hash` (or `state_hash_for_feature`) that folds the new inspector signal into the state hash (e.g. an additional `hasher.update(&[<signal> as u8])` for booleans, or a typed write for richer signals). This is the property [FT-138](FT-138)'s TC-349 was the silent-regression guard for — an implementer who adds the classifier row but forgets the hash update would still pass the precedence/positive/negative TCs and only break things in live drives by false-positiving the cycle detector across legitimate lifecycle transitions.
   - `derived_from: ["inspector_trait_method", "classifier_row"]`.
   - Model binding: `openai/code-small`.
   - Prompt template: `templates/extend_planner_classifier/state_hash_update.j2`.

6. **`unit_tests`** (cell type: code-specialist)
   - Artifact type: rust-test-fragment.
   - Emits: 4 unit tests in the planner module's `tests` block, naming-pattern locked by the audit:
     1. `precedence_*` — a higher-precedence row still wins over the new row when both fire (the FT-138 TC-345 pattern: `tcs = SomeUnready` AND the new signal both true → existing row's `Stuck` wins).
     2. `positive_*` — the new row fires for the canonical fixture (the FT-138 TC-346 pattern: signal true → declared `Action::*`).
     3. `negative_*` — existing behaviour preserved when the new signal is false (the FT-138 TC-347 regression-guard pattern: signal false → existing downstream action fires).
     4. `state_hash_*` — two `classify_and_hash` calls differing only in the new signal produce different hashes (the FT-138 TC-349 silent-regression guard, generalised).
   - `derived_from: ["inspector_trait_method", "classifier_row", "state_hash_update"]`.
   - Model binding: `openai/code-small`.
   - Prompt template: `templates/extend_planner_classifier/unit_tests.j2`.

**Coherence audit script:** `scripts/checks/cluster-audit-extend-planner-classifier.py`. Implements 6 checks (detailed under §Behaviour §Phase 2 below). Exit 0 = pass; exit 1 = audit failure with stderr describing which check failed and on which cell pair; exit 2 = unrunnable (missing input). Receives the 6 cell-output paths as CLI args.

### State

- New on-disk (substrate): this feature_spec's body declaring the cluster + a registry entry in `crates/decision-cli/src/core/task_type/registry.rs` (the FT-139 `lazy_static` map gains a second key `"extend-planner-classifier"`).
- New on-disk (prompt templates): 5 Jinja-style files under `crates/decision-cli/src/core/task_type/templates/extend_planner_classifier/` (one per LLM-driven cell; `inspector_default_impl` is deterministic and uses a render-only template).
- New on-disk (audit): `scripts/checks/cluster-audit-extend-planner-classifier.py`.
- Preserved on-disk: FT-139's substrate; the broad-worker fallback; every existing planner action; both planners' existing classifier tables; the `GraphInspector` trait surface; the orchestration store schema; every other TaskType in the registry.
- No orchestration-store schema change; no on-disk artifact schema change.

### Behaviour

#### Phase 1 — Register the TaskType in the FT-139 substrate

1. Add `"extend-planner-classifier"` to the static `TaskTypeRegistry` populated at startup in `crates/decision-cli/src/core/task_type/registry.rs`.
2. `TaskTypeDecl` is constructed with the 6 cells listed under §Outputs, each carrying its `derived_from` (`Cluster::topo_order` yields the canonical order `[inspector_trait_method, inspector_default_impl, inspector_production_impl, classifier_row, state_hash_update, unit_tests]`, modulo `inspector_default_impl` and `inspector_production_impl` being commutable since both only derive from `inspector_trait_method`).
3. Coherence audit spec: `script_path: scripts/checks/cluster-audit-extend-planner-classifier.py`, `timeout_seconds: 30`.

#### Phase 2 — Coherence audit script

`scripts/checks/cluster-audit-extend-planner-classifier.py` runs once after all 6 cells emit and asserts:

1. **Return-type triple agreement** — `inspector_trait_method`'s signature, `inspector_default_impl`'s body, and `inspector_production_impl`'s production override all agree on the `Result<<RetType>, InspectError>` shape. Regex extract the `<RetType>` from each emitted file; fail if the three differ.
2. **Method-name exact match in classifier** — `classifier_row` references the inspector method by the exact name emitted in `inspector_trait_method`. Regex match `self.inspector.<name>(` against the trait method's `fn <name>(` name.
3. **State-hash includes the new signal** — `state_hash_update`'s emitted fragment's hash-input writes (e.g. `hasher.update(...)` or equivalent) reference the new field/variable name. Regex assert the new signal name appears in the hasher's write region.
4. **Unit-tests naming pattern** — `unit_tests` emits ≥ 4 `#[test]` functions whose names match the canonical patterns: `precedence_*`, `positive_*`, `negative_*`, `state_hash_*`.
5. **Classifier row position documented relative to named adjacent rows** — `classifier_row`'s output includes a comment of the form `// FT-XXX / ADR-YYY: above <row_above>, below <row_below>`, and the audit additionally asserts the row's enum match arms appear between the two named existing rows by reading the file's match order (text-order check on the merged planner file).
6. **Cluster boundary — distinct from `extend-role-catalog-seed`** — every cell's output target path matches `**/planner.rs` OR `**/inspect_dor.rs` OR `**/inspect.rs` OR `**/registry.rs` OR `**/templates/extend_planner_classifier/**` OR `**/scripts/checks/cluster-audit-extend-planner-classifier.py`. NO output may target `**/seeds.rs` (the `extend-role-catalog-seed` cluster's signature surface). This rejects cross-cluster contamination at audit time.

Fail-loud: any check failing aborts the cluster, rolls back the worktree write (per FT-139 §Phase 2 §4), and surfaces a `ClusterAuditFailed { check, detail }` outcome with the failing check identifier verbatim.

#### Phase 3 — Tests

The 4 TCs listed below; all live in this feature_spec's `tests:` list:

1. **TC-355 (exit-criteria, cargo-test)** — Topo order: `Cluster::topo_order` over the 6 cells returns a valid topological order (`inspector_trait_method` first; `unit_tests` last; `inspector_default_impl` and `inspector_production_impl` both after `inspector_trait_method`; `classifier_row` after `inspector_production_impl`; `state_hash_update` after `classifier_row`; etc.) and the order is deterministic across runs. Runner: `cargo-test --package decision-cli --lib core::task_type::tests::planner_classifier_topo_order`.
2. **TC-356 (scenario, bash)** — Audit positive: all 6 cells emit fragments fitting between two named adjacent rows; the audit script returns exit 0. Synthetic-fixture runs `scripts/checks/cluster-audit-extend-planner-classifier.py` against a hand-authored matching-shape cell-output set.
3. **TC-357 (scenario, bash)** — Audit type-mismatch failure: a fixture where `inspector_trait_method` declares `Result<bool, InspectError>` but `inspector_production_impl` returns `Result<u32, InspectError>` triggers audit failure with the check-1 identifier (return-type triple agreement) verbatim on stderr.
4. **TC-358 (scenario, bash)** — Audit state-hash-missing failure: a fixture where `state_hash_update` emits the cell body but the new signal's name does NOT appear in the hasher's write region triggers audit failure with the check-3 identifier (state-hash includes the new signal) verbatim on stderr. This is the [FT-138](FT-138) TC-349 silent-regression guard, generalised — the audit catches it at cluster time, before the false-positive cycle detection ships.

### Invariants

- **Broad-worker fallback is non-optional.** A feature without `task_type: extend-planner-classifier` (or with an unknown value) falls through to the broad code-writer per FT-139 §Phase 2 §1. The classifier branch is purely additive at higher precedence.
- **Per-cell model binding.** Each cell resolves its own capability binding via the FT-067/068 resolver; no hardcoded model_ids or endpoints. `inspector_default_impl`'s deterministic-template binding is explicit (a `dec:capability:none` mechanical capability), not a missing field.
- **`derived_from` is data, not emergent.** The dispatcher honours the order; the topo-sort failure on cycles is a startup error per FT-139 §Phase 1 §3.
- **Cluster atomicity.** All 6 cells emit + audit passes + finalize commits, or the worktree is rolled back. Per FT-139 §Phase 2 §4.
- **Audit failure is loud and specific.** `ClusterAuditFailed` surfaces the failing check's identifier (one of the 6 listed under §Behaviour §Phase 2) verbatim in drive history. The operator can map it back without grepping.
- **Distinct from `extend-role-catalog-seed`.** Check 6 of the audit enforces this at cluster time. The two TaskTypes share the "small Rust-surface extension" shape; the audit boundary keeps the registry clean.

### Error handling

- **Cycle in `derived_from`** → `PlanError::ClusterCycle { cycle_path }` at TaskType registration time. (Cannot occur for the 6 cells as declared — defense-in-depth.)
- **Unknown TaskType in `task_type:` field on a consuming feature** → classifier returns `None`, falls through to broad worker per FT-139.
- **Cell emit fails** (LiteLLM error, file write error) → cluster aborts at that cell; cells already emitted roll back via worktree reset; emit `ClusterCellFailed { cell }`.
- **Audit script exit 2 (unrunnable)** → `ClusterAuditUnrunnable` outcome; same rollback semantics as audit-failed but a distinct outcome so the operator fixes the audit harness, not the cluster output.
- **Target planner ambiguity** (consuming feature_spec doesn't declare whether `FeatureReadyPlanner` or `FeatureShipPlanner` is the target) → the `classifier_row` cell receives the planner name as a required render variable; missing → cell-render error before LLM dispatch.

### Boundaries

- **In scope.** TaskType declaration (this body + registry entry); 5 prompt templates + 1 deterministic template for the 6 cells; coherence audit script; 4 TCs (topo + positive audit + type-mismatch audit + state-hash-missing audit).
- **Out of scope.** Other TaskType clusters (drafted in parallel under FT-140..FT-142 + sibling FT-TT-* slices). Promotion of TaskType + Cell to first-class product-cli artifact types — FT-139 holds the convention; promotion lands as a later cluster. Mixed-feature composition (a feature that extends both planners in one drive) — v1 dispatches the first matched TaskType. Backfilling FT-138, FT-131, or FT-119 into the cluster pattern (they already shipped through the broad worker). Cell-level retry. Audit checks beyond the 6 listed (e.g. SPARQL-syntax validation of the production impl's query body) — drafted as a follow-on if the prototype audit proves insufficient.

## Out of scope

- Other TaskType clusters (FT-140..FT-142 and sibling FT-TT-* slices).
- TaskType + Cell as first-class product-cli artifact types.
- Mixed-TaskType composition (extend both planners in one drive).
- Backfilling FT-138 / FT-131 / FT-119 into the cluster pattern.
- Cell-level retry or partial-cluster resume.
- SPARQL-body audit for `inspector_production_impl` (separate slice if the v1 audit proves insufficient).
- LLM-based selection of which planner to extend (operator declares it in the feature_spec).
- UI / `dec drive show` rendering of this cluster's outcomes (covered by FT-139's substrate work).
