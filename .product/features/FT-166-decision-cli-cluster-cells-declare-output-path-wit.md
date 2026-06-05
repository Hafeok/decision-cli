---
id: FT-166
title: 'decision-cli: cluster cells declare output_path with parameter substitution; TaskTypes declare parameters; per-feature values from task-types.toml'
phase: 4
status: complete
depends-on:
- FT-139
- FT-165
adrs:
- ADR-080
tests:
- TC-407
- TC-408
- TC-409
- TC-410
domains:
- api
domains-acknowledged: {}
---

## Description

Fourth and structural cluster fix after [FT-163](FT-163) (framing), [FT-164](FT-164) (turn cap), [FT-165](FT-165) (prompt). Witnessed by the final FT-147 retry where the worker emitted all 6 cells of [FT-141](FT-141)'s `add-artifact-type` cluster at **codebase-prescribed paths** (`crates/decision-cli/src/core/ontology/archetype/emitter.rs` etc.) — exactly what the spec said to do — while the cluster harness kept looking at its own **flat sandbox convention** (`emitter.rs` in sandbox root). 1,189 lines of usable code stranded by a path-mapping mismatch.

The cluster's flat-path convention dates to the FT-139 prototype when every cluster emitted one flat file per cell (`add-judge-worker`'s `agent_loop.py`, `add-cli-subcommand`'s `clap_args_module.rs`). It was reasonable for small clusters. Once the cluster is asked to ship features whose §Outputs prescribes a directory tree, the flat convention forces the worker into a path conflict it cannot resolve without re-prompting.

**Structural fix: cells declare their output_path with `{parameter}` placeholders, TaskTypes declare their parameters, per-feature parameter values land in `.dec/task-types.toml`.** Backwards-compatible — cells with empty `output_path` keep using the existing flat convention; FT-145's `add-cli-subcommand` cluster works unchanged.

## Functional Specification

### Inputs

- `crates/decision-cli/src/core/task_type/types.rs` — `CellDecl`, `TaskTypeDecl` ([FT-139](FT-139) substrate).
- `crates/decision-cli/src/core/task_type/registry.rs` — `add-artifact-type` declaration.
- `crates/decision-cli/src/features/drive/cluster_dispatch.rs` — flat-path resolution in `cell_filename`, `build_cell_bundle` prompt, `emit_llm_cell` read-back.
- `crates/decision-cli/src/features/drive/planners/feature_ship.rs` — `.dec/task-types.toml` readers (routing + FT-164 `read_max_turns_for_task_type`).
- Witnessed FT-147 emission at the spec-prescribed paths (the 11-file tree the worker correctly produced).

### Outputs

**Substrate type extensions:**

```rust
pub struct CellDecl {
    // existing fields...
    /// FT-166: workspace-relative output path with optional {parameter}
    /// placeholders resolved at dispatch time. Empty path → fall back to
    /// the FT-139 flat convention `<cell_name>.<ext>` via cell_filename.
    pub output_path: PathBuf,
}

pub struct TaskTypeDecl {
    // existing fields...
    /// FT-166: parameter declarations cells may reference in their
    /// output_path. Empty list → no parameters; cluster falls back to
    /// flat-path convention regardless of feature.
    pub parameters: Vec<TaskTypeParameter>,
}

pub struct TaskTypeParameter {
    pub name: String,
    pub description: String,
    pub default: Option<String>,
}
```

**Per-feature parameter values** in `.dec/task-types.toml` (additive — does NOT break the existing `[features]` routing table):

```toml
[features]
"FT-147" = "add-artifact-type"

# FT-166: per-feature parameter values consumed by cluster_dispatch
# when resolving cell output_paths. Optional — features without a
# [parameters."<id>"] table fall back to TaskTypeParameter defaults.
[parameters."FT-147"]
artifact_name = "archetype"

[task_types.add-artifact-type]
max_turns = 40
```

**Reader helper** at `feature_ship.rs` next to existing readers:

```rust
pub fn read_parameters_for_feature(
    cwd: &Path,
    feature_id: &str,
) -> BTreeMap<String, String>
```

Returns an empty map for any IO/parse failure (defensive — cluster dispatch never errors over misconfig).

**`add-artifact-type` re-declared** with paths from the witnessed FT-147 emission:

```rust
cell("rust_struct",        "rust-source", &[], "crates/decision-cli/src/core/ontology/{artifact_name}.rs"),
cell("shacl_shape",        "turtle",      &["rust_struct"], "crates/decision-cli/src/core/ontology/shapes/{artifact_name}.shacl.ttl"),
cell("iri_module_consts",  "rust-source", &["rust_struct"], "crates/decision-cli/src/core/vocab/{artifact_name}.rs"),
cell("parser",             "rust-source", &["rust_struct", "iri_module_consts"], "crates/decision-cli/src/core/ontology/{artifact_name}/parser.rs"),
cell("emitter",            "rust-source", &["rust_struct", "iri_module_consts"], "crates/decision-cli/src/core/ontology/{artifact_name}/emitter.rs"),
cell("round_trip_tests",   "rust-source", &["rust_struct", "shacl_shape", "parser", "emitter"], "crates/decision-cli/src/core/ontology/{artifact_name}/tests.rs"),
```

Plus `parameters: vec![TaskTypeParameter { name: "artifact_name", description: "Snake-case identifier for the artifact type (e.g. 'archetype', 'feedback', 'capability')", default: None }]`.

**Audit script update** at `scripts/checks/cluster-audit-add-artifact-type.py`: walk recursively (`Path.rglob` instead of `Path.glob`) so the script finds emitted files at the codebase-shaped paths, not just sandbox-root.

### State

- **Modified on-disk:**
  - `crates/decision-cli/src/core/task_type/types.rs` — `CellDecl`, `TaskTypeDecl` extensions + new `TaskTypeParameter` struct.
  - `crates/decision-cli/src/core/task_type/registry.rs` — `add-artifact-type` re-declaration + helper updates.
  - `crates/decision-cli/src/features/drive/cluster_dispatch.rs` — path resolution + read-back + prompt injection.
  - `crates/decision-cli/src/features/drive/planners/feature_ship.rs` — new `read_parameters_for_feature` helper.
  - `scripts/checks/cluster-audit-add-artifact-type.py` — recursive glob.
  - `.dec/task-types.toml` — `[parameters."FT-147"] artifact_name = "archetype"` block.
- **No new artifact types, no schema change, no orchestration-store mutation.**

### Behaviour

1. **Path resolution at cluster_dispatch::run start:**
   - Resolve `parameters` for the feature via `read_parameters_for_feature(workdir, feature_id)`.
   - For each cell, compute resolved path = substitute `{<param>}` in `cell.output_path` against the param map.
   - If `cell.output_path` is empty → fall back to `cell_filename` flat convention (FT-145 cluster works unchanged).
2. **Prompt injection:** `build_cell_bundle` emits "Write at: `<resolved_path>`" verbatim in the §"Your task" section, so the worker sees the exact path including subdirectories.
3. **Read-back:** `emit_llm_cell` reads from `cluster_dir.join(resolved_path)` (creating parent dirs first). Mechanical cells write at the same resolved path.
4. **Audit invocation unchanged:** the script receives the cluster sandbox dir as `$1` and walks recursively. The script changes (one-line glob update) are the only audit-side adaptation.
5. **Defaults:** when a TaskType declares parameters with defaults and the feature's `.dec/task-types.toml` doesn't override, the default is used. Parameters without defaults that are also unset → dispatch refuses with a clean error naming the missing parameter (per-task-type required-parameter check).

### Invariants

- **Empty `output_path` ≡ flat convention.** No regression for any existing cluster (add-judge-worker, add-author-worker, add-cli-subcommand, extend-planner-classifier, extend-role-catalog-seed) until their TaskType re-declarations land — they keep working with their flat-path emissions.
- **Substitution is one-pass.** No nested `{{parameter}}` expansion; placeholders are literal `{name}` matched against the param map.
- **Required parameters surface early.** Missing required parameters fail at the start of `run` before any worker dispatch — operator gets a clean diagnostic, not a confusing mid-cluster failure.
- **Parameter values are strings.** Per the TOML schema; no type coercion (numeric/bool overrides would be a separate slice).
- **Path traversal is forbidden.** Resolved paths must stay inside the sandbox — `..` segments after substitution fail dispatch with a clear error.

### Error handling

- **Missing required parameter** → `ClusterDispatchError::MissingRequiredParameter { task_type, parameter }`, surfaced before any cell dispatch.
- **Parameter value contains `..`** → `ClusterDispatchError::PathTraversalRejected { cell, resolved_path }`. Safety guard.
- **TOML parse failure on parameter read** → degrades to empty map (cells with defaults still work; cells without defaults surface the missing-parameter error).
- **Worker writes at wrong path despite prompt** → existing FT-146 "did not produce <path>" error, now naming the resolved path verbatim.

### Boundaries

- **In scope.** Substrate type extensions, parameter reader, path-resolution in cluster_dispatch, `add-artifact-type` re-declaration, audit script recursive-glob update, 4 TCs.
- **Out of scope.** Migrating other TaskTypes (add-judge-worker, add-author-worker, add-cli-subcommand, extend-planner-classifier, extend-role-catalog-seed) to declare output_path — they keep flat-path. Once FT-147 ships cleanly through the new substrate, follow-on slices can migrate the rest if needed. Parameter type coercion (int / bool overrides). Per-cell parameter overrides (every cell of a task type shares the same param map). CLI flag override of parameters (`dec drive ship FT-147 --param artifact_name=other`). Template engines beyond literal substitution (no Handlebars, no Jinja). Auto-apply from sandbox to real codebase paths after successful dispatch — operator still copies by hand.

## Out of scope

- Migrating other TaskTypes to declare output_path (follow-on slice once shape is proven).
- Parameter type coercion.
- Per-cell parameter overrides.
- CLI flag override of parameters.
- Template engines beyond literal `{name}` substitution.
- Auto-apply from sandbox to real codebase.
