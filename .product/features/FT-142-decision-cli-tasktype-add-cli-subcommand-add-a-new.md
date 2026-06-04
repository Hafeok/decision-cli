---
id: FT-142
title: 'decision-cli: TaskType add-cli-subcommand — add a new dec subcommand with clap args + handler + integration test + optional MCP twin'
phase: 5
status: planned
depends-on: []
adrs:
- ADR-080
- ADR-072
tests:
- TC-358
- TC-359
- TC-360
- TC-361
domains:
- api
domains-acknowledged:
  observability: 'FT-142 ships 4 TCs (TC-A exit-criteria + TC-B/C/D scenarios) satisfying ADR-072. ADR-072 spans api + observability. Observability concerns are covered by TC-B (positive audit case asserts the structural checks fire and pass — observable as exit 0 with no stderr), TC-C (negative case asserts the audit surfaces the failing check identifier `flags_tested` verbatim on stderr — the audit''s "teeth" property observable), and TC-D (negative case asserts the audit surfaces the failing check identifier `integration_test_path` verbatim on stderr — the structural discriminator against artifact-type / worker-task misclassification observable). The audit''s per-check stderr discipline is the observability surface: every cluster failure carries a grep-able check identifier so operators can map outcomes to root cause without rerunning. Explicit acknowledgement per ADR-072 review gate.'
---

## Description

TaskType declaration for **`add-cli-subcommand`** — the witnessed self-implementation pattern in which a feature adds a single new `dec <verb>` (or `dec product <verb>`) subcommand together with its full surface: clap derive args, handler module, registration wiring, optional MCP twin, integration test, and help doc string. Authored per [ADR-080](ADR-080)'s decision to apply DDD's task/cell decomposition to decision-cli's self-implementation pipeline; uses the [FT-139](FT-139) substrate (`TaskTypeDecl`, `CellDecl`, `Cluster::topo_order`, `cluster_dispatch::run`, coherence-audit pattern) as the foundation.

Witnessed motivating gap (all shipped, diffs 80% identical):

| Feature | Subcommand added |
|---|---|
| [FT-038](FT-038) | `dec verify env new` |
| [FT-049](FT-049) | `dec verify graph generate` |
| [FT-099](FT-099) | `dec verify graph run`, `dec verify feature` |
| [FT-109](FT-109) | `dec loop` |
| [FT-110](FT-110) | `dec drive` |

Each of these shipped through the broad code-writer and re-derived the same skeleton from scratch: a clap derive struct under `crates/decision-cli/src/cli/<command>.rs`, a handler module under `crates/decision-cli/src/features/<feature_dir>/`, the one-line wiring into the top-level CLI dispatcher in `crates/decision-cli/src/main.rs` (or `cli/mod.rs`), an integration test under `crates/decision-cli/tests/<command>.rs` using `assert_cmd`, an optional MCP tool registration when the subcommand surfaces through the MCP server, and a `///` doc-comment / extended-help block that clap renders. Routing each future CLI subcommand through a one-shot broad-worker dispatch is unaudited and unnecessary; this TaskType captures the boilerplate as a cluster of six typed cells with a per-task coherence audit that asserts the structural agreements no broad-worker shared context guaranteed implicitly.

This slice is **spec authoring only** — it declares the TaskType + Cell cluster + coherence audit in the `FT-TT-add-cli-subcommand` body convention; it does not implement the cluster_dispatch executor (FT-139), and it does not implement any specific CLI subcommand. Acceptance is "the TaskType is registered, the audit script exists, and the four TCs pass."

## Functional Specification

### Inputs

- The FT-139 substrate: `crates/decision-cli/src/core/task_type/` types (`TaskTypeDecl`, `CellDecl`, `Cluster::topo_order`, `CoherenceAuditSpec`) and the static TaskType registry populated at startup.
- The FT-139 `cluster_dispatch::run` executor, which walks Cells in `derived_from` order and invokes the coherence audit after all cells emit.
- The classifier branch in `features/drive/planners/feature_ship.rs` that reads `task_type:` from the consuming feature's front-matter and dispatches the matched TaskType's cluster.
- Five reference implementations of the witnessed pattern: FT-038, FT-049, FT-099, FT-109, FT-110 — used as the source of truth for the cluster's cell shapes and the audit's expected contract surface (each touched the same six surfaces).
- The capability resolver from [FT-067](FT-067)/[FT-068](FT-068) — each cell's model binding plugs in here at dispatch time.

### Outputs

**TaskType declaration:**

- `.product/features/FT-TT-add-cli-subcommand.md` (using the `FT-TT-<name>` convention from ADR-080 §Decision §1) declaring:
  - Recognition signature: `task_type: add-cli-subcommand` in the consuming feature's front-matter.
  - Cell cluster (with `derived_from` order — six cells total).
  - Coherence audit: pointer to `scripts/checks/cluster-audit-add-cli-subcommand.py`.
  - Conditional cell flag: `surfaces_via_mcp: bool` on the cluster invocation controls whether the optional `mcp_tool_shim` cell is included.

**The six cells:**

1. **`clap_args_module`** (role: `code-specialist`, model: `openai/code-small`).
   - Artifact type: Rust source file.
   - Output path: `crates/decision-cli/src/cli/<command>.rs`.
   - Emits: a clap `derive(Args)` struct (or `derive(Subcommand)` enum if multi-arity), with `///` doc comments on every field.
   - `derived_from: []` (root cell — the contract everyone else honours).
   - Prompt template: `crates/decision-cli/src/core/task_type/templates/add_cli_subcommand/clap_args.j2`.

2. **`handler_module`** (role: `code-specialist`, model: `openai/code-small`).
   - Artifact type: Rust source file.
   - Output path: `crates/decision-cli/src/features/<feature_dir>/mod.rs`.
   - Emits: `pub fn run(args: Args, ctx: &Context) -> Result<ExitCode>` whose body references every `pub` field on the clap args struct.
   - `derived_from: [clap_args_module]`.
   - Prompt template: `…/templates/add_cli_subcommand/handler.j2`.

3. **`registration_wiring`** (role: `mechanical`, model: `deterministic template — one-liner`).
   - Artifact type: Rust source patch (line-level addition into the dispatcher).
   - Output path: `crates/decision-cli/src/main.rs` (or `crates/decision-cli/src/cli/mod.rs`, whichever owns the top-level dispatcher).
   - Emits: the one-line `use crate::cli::<command>::Args;` import, the `use crate::features::<feature_dir>::run as <command>_run;` import, and the dispatcher arm calling the handler.
   - `derived_from: [clap_args_module, handler_module]`.
   - No LLM call — deterministic template substitution.

4. **`mcp_tool_shim`** (role: `code-specialist`, model: `openai/code-small`) — **OPTIONAL**, included only when the cluster invocation passes `surfaces_via_mcp: true`.
   - Artifact type: Rust source file.
   - Output path: `crates/decision-cli/src/core/mcp/<command>.rs` (and the registration line in `core/mcp/mod.rs`).
   - Emits: the MCP write or read tool registration that calls `handler_module::run` after schema-validating the JSON payload.
   - `derived_from: [handler_module]`.
   - Prompt template: `…/templates/add_cli_subcommand/mcp_tool_shim.j2`.

5. **`integration_test`** (role: `code-specialist`, model: `openai/code-small`).
   - Artifact type: Rust integration test file.
   - Output path: `crates/decision-cli/tests/<command>.rs`.
   - Emits: an `assert_cmd`-driven test that exercises every advertised flag combination from the clap args struct (one `#[test]` per advertised flag/positional at minimum).
   - `derived_from: [clap_args_module, handler_module]`.
   - Prompt template: `…/templates/add_cli_subcommand/integration_test.j2`.
   - **Path discipline:** must land under `crates/decision-cli/tests/` (integration test root), NOT under `crates/decision-cli/src/` (which would be a unit test and would conflict with the artifact-type / worker task types whose tests live under `src/`). The audit asserts this.

6. **`help_doc_string`** (role: `mechanical`, model: `deterministic template`).
   - Artifact type: Rust doc-attribute block.
   - Output path: a `#[doc = "…"]` attribute attached to the clap `Args` struct in `crates/decision-cli/src/cli/<command>.rs` (or a sibling `extended_help.md` rendered via `after_help = include_str!(...)`).
   - Emits: the top-level doc string for the subcommand plus the extended-help body listing every flag.
   - `derived_from: [clap_args_module]`.
   - No LLM call — deterministic template that walks the clap args struct's fields and emits one line per flag.

**Coherence audit for `add-cli-subcommand`:**

`scripts/checks/cluster-audit-add-cli-subcommand.py` runs once after all (5 or 6) cells emit and asserts the six checks listed below. Each check that fails aborts the cluster (rolls back the worktree write) and surfaces `ClusterAuditFailed { check, detail }`. Checks:

1. **`fields_used`** — every `pub` field on `clap_args_module`'s struct (parsed via regex over `pub <ident>: <type>,` lines) is referenced at least once in `handler_module`'s function body (regex match against `args.<field_name>`).
2. **`flags_tested`** — every flag/positional advertised in `clap_args_module` (every `pub` field) appears in at least one `integration_test` scenario (regex search over the test file for the long-flag form, e.g. `--<field-name>` with `_` → `-` normalisation, or the positional's variable name).
3. **`flags_documented`** — every flag in `clap_args_module` has a `///` doc comment AND appears verbatim (long-flag form) in the `help_doc_string` cell's output.
4. **`wiring_imports_both`** — `registration_wiring`'s emitted patch imports BOTH `clap_args_module`'s `Args` type AND `handler_module`'s `run` function (regex search for `use crate::cli::<command>::` and `use crate::features::<feature_dir>::`).
5. **`mcp_calls_handler`** — if `mcp_tool_shim` is present, it imports `handler_module::run` and invokes it (regex search for both an import line and an invocation `run(args, &ctx)` or equivalent).
6. **`integration_test_path`** — at least one file emitted by the cluster lives under `crates/decision-cli/tests/` (path glob `crates/decision-cli/tests/*.rs`). This check is the structural discriminator that catches misclassification with the **artifact-type** task type (whose tests live under `crates/decision-cli/src/.../tests.rs` as unit tests) and the **add-judge-worker** / **add-author-worker** task types (whose tests live under `workers/<name>/tests/test_<name>.py`).

The audit is implemented as a single Python script (stdlib only, no decision-cli dep) invoked by `cluster_dispatch::run` after the cells emit. Exit 0 = pass; exit 1 = audit failure with stderr describing which check failed and why (`check=fields_used field=foo` form so the operator can grep); exit 2 = unrunnable (missing input file).

### State

- New on-disk (task type declaration): `.product/features/FT-TT-add-cli-subcommand.md`.
- New on-disk (audit script): `scripts/checks/cluster-audit-add-cli-subcommand.py`.
- New on-disk (prompt templates): `crates/decision-cli/src/core/task_type/templates/add_cli_subcommand/{clap_args,handler,mcp_tool_shim,integration_test}.j2` plus the deterministic templates for `registration_wiring` and `help_doc_string`.
- Updated in-source (registry): the TaskType registry from FT-139 gains an `add-cli-subcommand` entry pointing at the cells above and the audit script.
- Preserved: every existing planner action; the broad-worker dispatch path (unknown / `task_type:` absent still falls through); the existing CLI subcommand surface.
- No orchestration-store schema change; no on-disk artifact schema change.

### Behaviour

#### Phase 1 — Declare the `add-cli-subcommand` TaskType

1. Create `.product/features/FT-TT-add-cli-subcommand.md` with front-matter `id: FT-TT-add-cli-subcommand`, `kind: task-type` (informational), `phase: 5`, `domains: [api, observability]`, body declaring the six cells with their `derived_from` edges, model bindings, and prompt template paths exactly as listed in §Outputs.
2. Register the TaskType in the FT-139 static registry: `register("add-cli-subcommand", TaskTypeDecl { name, cells, coherence_audit })`.
3. The recognition signature is `task_type: add-cli-subcommand` in the consuming feature's front-matter; the classifier (FT-139 phase 2) matches on that string.

#### Phase 2 — Author the six prompt templates

1. `templates/add_cli_subcommand/clap_args.j2` — generates a clap derive `Args` struct (or `Subcommand` enum for multi-arity verbs) given the feature spec's "Inputs" section as input.
2. `templates/add_cli_subcommand/handler.j2` — generates the `run` function signature + body skeleton, importing the `Args` type from the clap_args_module's emitted path; body must reference every advertised field.
3. `templates/add_cli_subcommand/mcp_tool_shim.j2` — generates the MCP tool registration (write or read tool, picked by a `mcp_kind` cluster-invocation field with default `read`).
4. `templates/add_cli_subcommand/integration_test.j2` — generates an `assert_cmd::Command::cargo_bin("dec")` test scaffold with one `#[test]` per advertised flag.
5. `registration_wiring` and `help_doc_string` use deterministic template substitution — no Jinja prompt, just a Rust string template walking the args struct AST that the upstream cells emitted.

#### Phase 3 — Implement the coherence audit script

1. `scripts/checks/cluster-audit-add-cli-subcommand.py` accepts the cell-output paths as CLI args (one path per emitted cell) and the optional flag `--surfaces-via-mcp` indicating whether to apply check 5.
2. Implements the 6 checks listed under §Outputs using stdlib regex only — no decision-cli dependency, no Rust parser dependency (the regex is sufficient at the structural granularity the audit needs).
3. Exit semantics: 0 / 1 / 2 as in §Outputs.

#### Phase 4 — Per-cell model binding

1. Each cell's `model_binding_capability_iri` resolves through the FT-067/068 capability resolver at dispatch time — the TaskType declaration carries the capability IRI, not the model id.
2. The four `code-specialist` cells (`clap_args_module`, `handler_module`, `mcp_tool_shim`, `integration_test`) bind to `openai/code-small` via the capability layer.
3. The two `mechanical` cells (`registration_wiring`, `help_doc_string`) are deterministic templates — no LLM call, no capability binding required; their "model binding" entry is `none` and the dispatcher knows to skip the LiteLLM call for them.

#### Phase 5 — Tests

1. **TC-A (exit-criteria, cargo-test)** — `Cluster::topo_order` over the six cells of `add-cli-subcommand` returns a deterministic, acyclic ordering. Asserts `clap_args_module` comes first; `registration_wiring`, `mcp_tool_shim`, and `integration_test` come after `clap_args_module` and `handler_module`; `help_doc_string` comes after `clap_args_module`. Asserts re-running the topo sort returns the same order (determinism).
2. **TC-B (scenario, bash, positive)** — The audit script passes on a synthetic positive fixture (a tempdir with the six cell outputs all internally consistent: every clap field used in the handler, tested in the integration test, documented in the help doc, wiring imports both, MCP shim calls the handler, integration test lives under `crates/decision-cli/tests/`).
3. **TC-C (scenario, bash, negative — missing flag test)** — The audit script FAILS with `check=flags_tested` when the synthetic fixture's integration test omits one of the flags advertised in the clap args module. Asserts exit 1 and the specific check identifier in stderr.
4. **TC-D (scenario, bash, negative — no integration test file)** — The audit script FAILS with `check=integration_test_path` when the synthetic fixture emits no file under `crates/decision-cli/tests/` (simulating misclassification as `add-artifact-type` or `add-judge-worker`, whose tests live elsewhere). Asserts exit 1 and the specific check identifier in stderr.

### Invariants

- **Distinct from artifact-type and worker task types.** The cluster MUST include an integration test under `crates/decision-cli/tests/` — this is the structural marker that distinguishes `add-cli-subcommand` from `add-artifact-type` (tests under `src/.../tests.rs` as unit tests) and from `add-judge-worker` / `add-author-worker` (tests under `workers/<name>/tests/`). TC-D pins this in the audit.
- **Cluster atomicity (inherited from FT-139).** Either all 5 or 6 cells emit + audit passes + finalize commits, or the worktree is rolled back. No partial-cluster artifacts in git history.
- **Audit failure is loud.** A `ClusterAuditFailed` outcome surfaces with the failing check identifier (one of `fields_used`, `flags_tested`, `flags_documented`, `wiring_imports_both`, `mcp_calls_handler`, `integration_test_path`) — the operator can grep without rerunning.
- **Per-cell model binding via capability resolver.** No hardcoded `openai/code-small` in the dispatcher; the TaskType declaration carries the capability IRI, the resolver picks the concrete model. Mirrors FT-067/068.
- **Optional cell discipline.** `mcp_tool_shim` is emitted iff the cluster invocation passes `surfaces_via_mcp: true`. When absent, the audit's `mcp_calls_handler` check is skipped; the other five checks still run.
- **Mechanical cells have no LLM call.** `registration_wiring` and `help_doc_string` are deterministic templates; their session record reflects "no model invocation" so the cluster's audit log distinguishes mechanical vs. LLM cell emissions.

### Error handling

- **Cycle in `derived_from`** → `PlanError::ClusterCycle` at TaskType registration time (FT-139 substrate catches this at startup, not at first dispatch).
- **Audit check `fields_used` fails** → `ClusterAuditFailed { check: "fields_used", detail: "<field_name> never referenced in handler" }`; rollback.
- **Audit check `flags_tested` fails** → `ClusterAuditFailed { check: "flags_tested", detail: "<flag_name> not exercised in integration test" }`; rollback.
- **Audit check `flags_documented` fails** → `ClusterAuditFailed { check: "flags_documented", detail: "<flag_name> missing /// doc OR absent from help_doc_string output" }`; rollback.
- **Audit check `wiring_imports_both` fails** → `ClusterAuditFailed { check: "wiring_imports_both", detail: "missing import of <module>" }`; rollback.
- **Audit check `mcp_calls_handler` fails** (only when `surfaces_via_mcp: true`) → `ClusterAuditFailed { check: "mcp_calls_handler", detail: "MCP shim missing import or invocation of handler::run" }`; rollback.
- **Audit check `integration_test_path` fails** → `ClusterAuditFailed { check: "integration_test_path", detail: "no file under crates/decision-cli/tests/ in emitted set" }`; rollback. This is the discriminator against `add-artifact-type` and `add-judge-worker` misclassification.
- **Audit script returns exit 2** → `ClusterAuditUnrunnable`; same rollback semantics as audit-failed, distinct outcome so the operator fixes the audit harness rather than the cluster output.
- **Capability resolver returns no binding for a code-specialist cell** → `ClusterDispatchError::NoCapabilityForCell { cell, role }`; cluster aborts before any cell emits.

### Boundaries

- **In scope.** TaskType declaration + 6-cell cluster + audit script + 4 prompt templates + 2 deterministic templates + registry entry referencing the FT-139 substrate; 4 TCs validating topo order + audit positive/negative cases.
- **Out of scope.** The cluster_dispatch executor itself (FT-139). Specific CLI subcommand implementations (each future `dec <verb>` rides this TaskType). The `surfaces_via_mcp` field's plumbing into the cluster invocation (the FT-139 dispatcher reads it; this slice only declares the cell's existence and the audit's conditional check). Embedding-similarity classification (ADR-080 v1 uses operator-declared `task_type:`). Backfilling FT-038 / FT-049 / FT-099 / FT-109 / FT-110 into the cluster pattern (they shipped through the broad worker; conversion is a follow-on). UI rendering of the cluster's per-cell outcomes in `dec drive show` (FT-139 follow-on).

## Out of scope

- The cluster_dispatch executor implementation (lives in FT-139's slice).
- Implementing any specific `dec <verb>` subcommand via this TaskType (each is its own feature, dispatched through this TaskType once FT-139 ships).
- Promotion of TaskType + Cell to first-class product-cli artifact types (ADR-080 §Decision §1 keeps them in feature_spec body convention for v1).
- LLM-based / embedding-similarity classification.
- Backfilling existing CLI subcommands (FT-038, FT-049, FT-099, FT-109, FT-110) through this cluster.
- Cell-level retry / partial-cluster resume.
- Per-cell secrets management beyond the capability resolver's existing path.
- UI for visualising `add-cli-subcommand` cluster outcomes in `dec drive show`.
