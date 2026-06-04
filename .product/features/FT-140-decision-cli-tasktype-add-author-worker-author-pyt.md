---
id: FT-140
title: 'decision-cli: TaskType add-author-worker — author Python workers that draft markdown bodies for engineering artifacts'
phase: 5
status: complete
depends-on: []
adrs:
- ADR-080
- ADR-072
tests:
- TC-350
- TC-351
- TC-352
- TC-353
domains:
- api
domains-acknowledged:
  observability: FT-140 ships 4 TCs (TC-A exit-criteria + TC-B/C/D scenarios) satisfying ADR-072's count rule on a feature spanning api + observability. The observability surface is covered by TC-B (positive audit run asserts the coherence audit script's PASS/FAIL stderr lines are observable and machine-parseable), TC-C (negative discriminator asserts the canonical `output is a verdict, not a draft` failure message surfaces verbatim — the audit's "teeth" property is observable through the failure detail), and TC-D (classifier branch asserts the `Action::DispatchCluster` decision is observable in the planner output for the new task type registry entry). The audit script's exit codes (0/1/2 mapped to ClusterAuditPassed/Failed/Unrunnable outcomes per FT-139) are the primary observability contract; this slice's three scenario TCs exercise each of those branches. Explicit acknowledgement per ADR-072 review gate.
---

## Description

The second TaskType declared under [ADR-080](ADR-080)'s task/cell decomposition substrate. Where [FT-139](FT-139) shipped the substrate plus `add-judge-worker` (the load-bearing prototype for workers that emit *verdicts* over existing content), this slice declares the parallel cluster for workers that *author* new content — Python workers whose output is a drafted markdown body for an engineering artifact (feature_spec, ADR, TC) plus the section-structure metadata used to validate the draft against ADR-055 / ADR-047 body conventions.

Witnessed motivating gap (from the planned roadmap):
- **FT-126 tc-author** (shipped) — drafts TC bodies from feature_spec excerpts. Proves the pattern at the simplest scale.
- **FT-129 spec-author** (planned) — drafts feature_spec bodies from request briefs. Same boilerplate as FT-126: `pyproject.toml`, agent loop calling LiteLLM with the canonical FT-123 shape, Pydantic Input (brief / gap context) and Output (drafted markdown + section metadata), system prompt under `src/<name>/prompts/`, capability binding, role catalog seed entry.
- **FT-130 adr-author** (planned) — drafts ADRs or acknowledgements for preflight gaps. Same shape again.

Each of these queued through the broad code-writer would re-derive 80% identical boilerplate per ADR-080's analysis. Routing them through a declared `add-author-worker` cluster makes the per-cell model binding explicit (the prompt cell can take a stronger reasoning model; the loop / models cells take code-small), audits the cluster's contract surface mechanically, and avoids re-spending the broad-worker dispatch on a recognized pattern.

**Explicit distinction from `add-judge-worker`.** A judge worker reads existing content and emits a structured verdict (`approve`/`reject`/`needs_work` + reasoning). An author worker reads a brief plus context and emits *drafted markdown* — a body that will become the body of a feature_spec / ADR / TC. The Output schema is the discriminator: judges have `verdict: str`; authors have `body_markdown: str` plus a `sections` map. The coherence audit asserts this distinction directly so that a misclassified cluster (a judge spec dispatched as an author, or vice versa) fails loudly at audit time rather than shipping a confidently-wrong worker.

One TaskType → one slice. Like FT-139, this is a single feature_spec carrying the cluster declaration, the coherence audit, and the classifier-side wiring for the new task type. It does NOT introduce a new top-level CLI verb; it extends the registry that FT-139 ships.

## Functional Specification

### Inputs

- The TaskType + Cell substrate from FT-139 (`crates/decision-cli/src/core/task_type/`): `TaskTypeDecl`, `CellDecl`, `Cluster::topo_order`, `CoherenceAuditSpec`. This slice plugs into the substrate; it does not re-author it.
- The cluster_dispatch executor and classifier branch from FT-139 (`features/drive/cluster_dispatch.rs`, `features/drive/planners/feature_ship.rs`). The classifier extension here is purely a new registry entry plus a new TaskType row matched against `task_type: add-author-worker` front-matter.
- The reference shipped author worker: `workers/tc-author/` (FT-126) — source of truth for cell shapes. FT-129 and FT-130 are the planned consumers and must be dispatchable via this cluster.
- Section conventions from [ADR-055](ADR-055) (feature_spec body shape) and [ADR-047](ADR-047) (ADR body shape) — referenced by the system prompt cell and verified by the coherence audit.
- The capability resolver from FT-067/068 — each cell's model binding plugs in here, identical to FT-139.
- The canonical FT-123 in-process LiteLLM call shape (`model=payload.model_id`, `base_url=LITELLM_BASE_URL`) — every author cluster's `agent_loop.py` MUST match.

### Outputs

**TaskType declaration:**

- `.product/features/FT-TT-add-author-worker.md` (using the `FT-TT-<name>` convention from ADR-080 §1 — not yet a first-class artifact type) declaring:
  - Recognition signature: `task_type: add-author-worker` in the consuming feature_spec's front-matter.
  - Cell cluster (in `derived_from` order):
    1. **`capability_binding`** — emits `workers/<name>/seed.nq`-style role + capability + binding quads (role IRI + capability IRI + endpoint + model_id). `derived_from: []`. Model: deterministic template — no LLM call; pure code generation from the TaskType registry's parameters.
    2. **`pydantic_io_models`** — emits `workers/<name>/src/<name>/models.py` with **Input** types (request brief, gap-context excerpt, surrounding feature context) and **Output** types: `body_markdown: str` (the drafted artifact body) plus `sections: dict[str, str]` (map of H2 heading → body of that section, used by the coherence audit to verify the prompt's section requirements match what the model is asked to emit). `derived_from: []`. Model: `openai/code-small` via the capability resolver.
    3. **`system_prompt`** — emits `workers/<name>/src/<name>/prompts/system.md`. Instructs the author to follow the spec body's H2 section conventions per ADR-055 (Description / Functional Specification / Out of scope) for feature_spec authors, ADR-047 (Context / Decision / Rejected alternatives / Consequences) for ADR authors. The system prompt's referenced H2 names MUST match the keys the Output schema's `sections` field is declared to contain. `derived_from: [pydantic_io_models]`. Model: `openai/reasoning-large` — author quality is where the heavier model spends its budget.
    4. **`agent_loop`** — emits `workers/<name>/src/<name>/agent/loop.py` with the canonical FT-123 LiteLLM call shape: `litellm.completion(model=payload.model_id, base_url=LITELLM_BASE_URL, ...)`. `derived_from: [pydantic_io_models, system_prompt]`. Model: `openai/code-small`.
    5. **`fixtures_example_inputs`** — emits `workers/<name>/tests/fixtures/example_<n>.json` files containing realistic Input payloads (a sample request brief, a sample gap excerpt, etc.) the author worker can be tested against. `derived_from: [pydantic_io_models]`. Model: deterministic template — generated from the input schema using a fixture-synthesizer template; no LLM call.
    6. **`unit_tests`** — emits `workers/<name>/tests/test_<name>.py`. Fixture-driven (loads `example_*.json` files), mocks LiteLLM at the boundary, runs the agent loop, asserts the Output body contains the H2 headings the system prompt declares. `derived_from: [pydantic_io_models, fixtures_example_inputs, system_prompt]`. Model: `openai/code-small`.
  - Coherence audit: pointer to `scripts/checks/cluster-audit-add-author-worker.py`.
- Registry entry: a new `TaskTypeDecl` in the FT-139 registry for `add-author-worker`, with the six cells listed above, their `derived_from` edges, their `model_binding_capability_iri`s, and their prompt template paths under `crates/decision-cli/src/core/task_type/templates/add_author_worker/`.

**Coherence audit for `add-author-worker`:**

The audit runs once after all six cells emit and asserts the cluster's contract holds. It MUST catch the cross-cell divergences a single broad-worker context would have caught implicitly. The six checks are:

1. **`agent_loop_calls_litellm_canonical`** — `loop.py` calls `litellm.completion` with `model=payload.model_id` and `base_url=LITELLM_BASE_URL`. Regex match over the file body for the canonical FT-123 shape; absence of either argument fails the check.
2. **`output_schema_has_body_and_sections`** — `models.py`'s Output type has both a `body_markdown: str` field AND a `sections` field whose type is a dict / `dict[str, str]` / `Mapping[str, str]`. AST-based check; absence of either field fails.
3. **`system_prompt_references_h2_sections`** — `system.md` references (in plain text) every H2 heading name the Output schema's `sections` field is documented to contain. The Output's `sections` field declares its keys via a class-level constant `EXPECTED_SECTIONS: list[str]` (convention enforced by the audit). Each name in that list must appear as a `## <name>` reference in the prompt.
4. **`fixtures_validate_against_input_schema`** — every `tests/fixtures/example_*.json` loads cleanly via the Input pydantic model's `model_validate`. A fixture that fails validation fails the check with the offending fixture path + the pydantic error.
5. **`unit_tests_construct_output_through_stubbed_loop`** — the unit test file contains a test that loads a fixture, constructs the Input, drives the agent loop with LiteLLM stubbed, receives an Output, and asserts the `body_markdown` contains every H2 heading in `EXPECTED_SECTIONS`. AST-based detection of the assertion pattern; absence fails the check.
6. **`output_is_draft_not_verdict`** — **the discriminator vs. `add-judge-worker`.** The Output type MUST have `body_markdown: str` and MUST NOT have a `verdict` field (any spelling: `verdict`, `Verdict`, `verdict_label`, etc.). A judge cluster's Output has `verdict: str`; an author's does not. This check catches the misclassification where a cluster authored under `add-author-worker` accidentally emits a judge-shaped Output (or vice versa). Fail-loud with the message `output is a verdict, not a draft — did you mean add-judge-worker?` to make the misclassification visible.

The audit is a Python script with no decision-cli dependency, receives the cell-output file paths as CLI args, exits 0 on pass, 1 on audit failure with a stderr line of the form `FAIL <check_name>: <detail>`, exits 2 if a required input is missing.

**Classifier branch + dispatcher:**

- No new code on the classifier side beyond a registry entry. The classifier extension from FT-139 already reads `task_type:` from the feature_spec front-matter and dispatches against the registry. This slice adds the `add-author-worker` entry; the classifier matches it the same way it matches `add-judge-worker`.
- `cluster_dispatch::run` is unchanged from FT-139; the new TaskType walks its six cells through the same executor.

### State

- New on-disk (task type declaration): `.product/features/FT-TT-add-author-worker.md`.
- New on-disk (audit script): `scripts/checks/cluster-audit-add-author-worker.py`.
- New on-disk (prompt templates): `crates/decision-cli/src/core/task_type/templates/add_author_worker/{capability_binding,pydantic_io_models,system_prompt,agent_loop,fixtures_example_inputs,unit_tests}.j2` (or equivalent template language; one file per cell).
- New in-binary (registry): one additional `TaskTypeDecl` for `add-author-worker` registered in the FT-139 task-type registry at startup.
- Preserved on-disk: the FT-139 substrate; the FT-139 `add-judge-worker` TaskType; the broad-worker fallback; verify-graph-author's capability binding pattern.
- No orchestration-store schema change; no on-disk artifact schema change.

### Behaviour

#### Phase 1 — Declare the TaskType in the feature_spec catalog

1. Create `.product/features/FT-TT-add-author-worker.md` with front-matter `id: FT-TT-add-author-worker`, `kind: task-type` (informational), `phase: 5`, and a body declaring the six cells listed in §Outputs with their `derived_from` edges, artifact paths, prompt template paths, and `model_binding_capability_iri` values.
2. The body cross-references this feature_spec (FT-140) as the implementing slice and ADR-080 as the governing decision.

#### Phase 2 — Register the TaskType in the Rust registry

1. Add `add_author_worker()` constructor to `crates/decision-cli/src/core/task_type/registry.rs` (or wherever FT-139's `add_judge_worker()` lives), returning a `TaskTypeDecl` populated with the six cells.
2. Wire the constructor into the static registry initializer.
3. Add prompt templates under `crates/decision-cli/src/core/task_type/templates/add_author_worker/`.

#### Phase 3 — Implement the coherence audit script

1. `scripts/checks/cluster-audit-add-author-worker.py` implements the six checks listed under §Outputs.
2. Script signature: `cluster-audit-add-author-worker.py --capability-binding <path> --pydantic-io-models <path> --system-prompt <path> --agent-loop <path> --fixtures-dir <path> --unit-tests <path>`.
3. Each check emits `PASS <check_name>` or `FAIL <check_name>: <detail>` to stderr.
4. Exit 0 if all six pass; exit 1 if any fail; exit 2 if a required input file is missing (unrunnable).
5. Pure Python stdlib + `pydantic` + `ast` — no decision-cli dependency.

#### Phase 4 — Tests

1. **TC-A (exit-criteria, cargo-test)** — `Cluster::topo_order` returns a deterministic, acyclic ordering for the `add-author-worker` cluster. Tests the substrate's contract against the six-cell `derived_from` graph; expected order: `capability_binding` (no deps), `pydantic_io_models` (no deps), `fixtures_example_inputs` (after models), `system_prompt` (after models), `agent_loop` (after models + prompt), `unit_tests` (after models + fixtures + prompt).
2. **TC-B (scenario, bash)** — Positive cluster: a synthetic fixture directory with all six cells emitting consistent contracts (matching field names, matching H2 sections, valid fixtures, draft-shaped Output). Runs the actual audit script and asserts exit 0.
3. **TC-C (scenario, bash)** — Negative cluster (the discriminator test): same fixture but with `models.py`'s Output declaring `verdict: str` instead of `body_markdown: str`. Runs the audit, asserts exit 1, asserts stderr contains the canonical `output is a verdict, not a draft` message. This is the test that proves the audit's "teeth" property — it catches the misclassification a broad-worker single context would have caught implicitly.
4. **TC-D (scenario, cargo-test)** — Classifier branch: `FeatureShipPlanner::classify` returns `Action::DispatchCluster { task_type_name: "add-author-worker" }` when the feature_spec front-matter carries `task_type: add-author-worker`; falls through to `Action::DispatchImplementer` (the broad-worker fallback) when the front-matter is absent or carries an unknown value. Asserts the classifier branch from FT-139 generalizes correctly to the new registry entry.

The 4-TC count is intentional and satisfies ADR-072: one exit-criteria (the topological ordering invariant), three scenarios (positive cluster, negative discriminator, classifier dispatch). Together they cover the audit's behavioural surface, its discriminator vs. add-judge-worker, and the registry / classifier integration.

### Invariants

- **Discriminator vs. add-judge-worker is structural.** The Output schema MUST carry `body_markdown: str`; it MUST NOT carry any field whose name normalizes to `verdict`. The coherence audit enforces this; a cluster declared under `add-author-worker` cannot ship with a judge-shaped Output even if every other cell is internally consistent.
- **All six cells must emit before audit runs.** The cluster is atomic per FT-139's invariants — either every cell emits + audit passes + finalize commits, or the worktree is rolled back.
- **`derived_from` graph is acyclic and respected.** `pydantic_io_models` has no deps; `capability_binding` has no deps; `fixtures_example_inputs` and `system_prompt` depend on `pydantic_io_models`; `agent_loop` depends on both `pydantic_io_models` and `system_prompt`; `unit_tests` depends on all three of `pydantic_io_models`, `fixtures_example_inputs`, `system_prompt`. The substrate's topo sort handles the actual ordering; the cluster declaration just states the edges.
- **Per-cell model binding via capability resolver.** No hardcoded model_ids in the cluster declaration; the cell's `model_binding_capability_iri` resolves to the capability binding at dispatch time, exactly per FT-139 and FT-067/068.
- **Broad-worker fallback remains non-optional.** Per ADR-080's escape-hatch principle, a feature_spec without `task_type:` (or with an unknown value) still routes to the broad code-writer.

### Error handling

- **Cycle in `derived_from`** — caught by FT-139's `Cluster::topo_order`; surfaces as `PlanError::ClusterCycle` at TaskType registration time. No change from FT-139.
- **Audit check failure** — surfaces as `ClusterAuditFailed { check, detail }` outcome; the cluster's worktree edits roll back. The `check` value is the canonical check name from §Outputs (one of the six).
- **Misclassification caught at audit** — same failure surface; the `check` field is `output_is_draft_not_verdict` and the `detail` includes the canonical hint pointing operator at `add-judge-worker`.
- **Fixture validation failure during audit** — `check = fixtures_validate_against_input_schema`; detail includes the offending fixture path and the pydantic error verbatim.
- **Missing input file (e.g. one cell did not emit)** — audit exits 2 (unrunnable); cluster outcome is `ClusterAuditUnrunnable`. Per FT-139, this is a distinct outcome from `ClusterAuditFailed` so operators can distinguish "audit found a problem" from "audit could not run".

### Boundaries

- **In scope.** The `add-author-worker` TaskType declaration (feature_spec catalog entry + registry entry + six prompt templates); the coherence audit script with its six checks (including the discriminator vs. `add-judge-worker`); 4 TCs (one exit-criteria + three scenarios) covering topo order, positive audit, negative discriminator, and classifier branch.
- **Out of scope.** Implementing FT-129 (spec-author) or FT-130 (adr-author) themselves — those are downstream consumers of this cluster and ship as their own feature_specs that carry `task_type: add-author-worker` in their front-matter. Promotion of TaskType + Cell to first-class product-cli artifact types (deferred per ADR-080 §Decision §1 to the future `add-artifact-type` cluster). Embedding-similarity or LLM-based classification (v1 uses operator-declared `task_type:` per FT-139). Mixed-feature composition (a feature that is both `add-author-worker` AND something else — v1 dispatches the first matched TaskType; multi-TaskType composition is a future slice). Backfilling FT-126 tc-author into the cluster pattern (tc-author already shipped through the broad worker; conversion is a separate cleanup).

## Out of scope

- Implementing FT-129 spec-author or FT-130 adr-author (those carry `task_type: add-author-worker` and dispatch via this cluster but are their own slices).
- Promotion of TaskType + Cell to first-class product-cli artifact types.
- Embedding similarity / LLM-based classifier extensions.
- Mixed-feature composition (multi-TaskType per feature).
- Backfilling tc-author or any existing author worker into the cluster pattern.
- Cell-level retry / partial-cluster resume.
- A separate "author quality" judge worker (that is `add-judge-worker` territory, e.g. FT-132 / FT-133 in the roadmap).
