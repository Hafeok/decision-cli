---
id: FT-156
title: 'decision-cli: archetype-pattern-extractor worker package — mine instance repos into archetype contracts and candidate TaskTypes'
phase: 5
status: planned
depends-on:
- FT-152
- FT-154
adrs:
- ADR-085
- ADR-080
tests: []
domains:
- api
domains-acknowledged: {}
---

## Description

A Python author-style worker that implements the **Pattern Extraction → Archetype Catalog Playbook v2**. Reads N instance repos of the same system kind and emits the archetype scaffolding: `archetype.yaml`, `application/contract.md`, `infrastructure/contract.template.md`, `task-types/{application,infrastructure}/`, candidate audits, and `EVIDENCE.md` with coverage honesty.

This is the broad-authority **explorer-and-typifier** worker from `briefs/pattern-extraction-playbook-v2.md`. It runs on the broad code-writer's authority surface ([FT-123](FT-123)) — not on a typed cell-cluster — because the *output* it produces is the type system itself. The playbook is direct (`§9.1`): "Emit only `status: candidate`. Promotion to `standard` is a gated human decision." This worker mints candidates; humans promote.

The worker also reads the [FT-154](FT-154) `TaskTypeCandidate` records accumulated from prior dispatches against the archetype. Candidates with `recurrence_hint: LikelyRecurring` are the primary input to the worker's "propose a new TaskType" path — the broad worker is closing the feedback loop on the catalog the dispatcher hits its edges against.

## Functional Specification

### Inputs

- A `Bundle` containing: target archetype id, N instance repo references (paths + commits), optional prior `archetype.yaml` for amendment mode, optional set of `TaskTypeCandidate` records to consider.
- LiteLLM proxy via the worker SDK. Capability binding: `deep-reasoning` (Anthropic Claude Sonnet/Opus per ADR-037). The worker exercises broad authority — it reads files, walks ASTs, queries the graph; the in-process LiteLLM-client agentic loop from [FT-123](FT-123) is the runtime.
- Read-only access to repo workspaces (the worker SDK's read-only tool surface — no writes outside the archetype output directory).

### Outputs

**Python worker package** at `workers/archetype-pattern-extractor/`:

```
workers/archetype-pattern-extractor/
├── pyproject.toml
├── src/archetype_pattern_extractor/
│   ├── __init__.py
│   ├── models.py                           # ExtractionInput / ExtractionOutput
│   ├── prompts/system.md                   # broad-authority system prompt
│   ├── agent/loop.py                       # in-process agentic loop with tool surface
│   ├── tools/                              # file read, AST walk, graph query — bounded by workspace
│   │   ├── repo_inspect.py
│   │   ├── ast_walk.py
│   │   └── candidate_query.py
│   └── main.py
└── tests/test_extractor.py
```

**Pydantic IO** (`models.py`):

```python
class InstanceReference(BaseModel):
    repo_path: Path
    commit: str
    description: str | None

class ExtractionInput(BaseModel):
    archetype_id: str
    instances: list[InstanceReference]       # ≥1; ≥3 strongly recommended per playbook §5
    prior_archetype: ArchetypeManifest | None
    task_type_candidates: list[TaskTypeCandidateRecord]
    extraction_mode: Literal["mint", "amend", "regression-test"]

class ExtractionOutput(BaseModel):
    archetype_manifest: ArchetypeManifest    # the archetype.yaml content
    application_contract_md: str
    application_conventions: dict[str, str]  # name → body
    infrastructure_contract_template_md: str
    infrastructure_conventions: dict[str, str]
    application_task_types: list[TaskTypeDraft]
    infrastructure_task_types: list[TaskTypeDraft]
    archetype_audits: list[AuditDraft]
    seam_audits: list[SeamAuditDraft]        # MUST be non-empty per ADR-084
    evidence: EvidenceReport                 # coverage honesty, instance variance, contract invariance
    rejected_patterns: list[RejectedPattern] # patterns observed but not minted, with reason

class EvidenceReport(BaseModel):
    instances: list[InstanceReference]
    archetype_layer_estimate: float
    application_contract_held_invariant: bool
    instance_variance: Literal["low", "medium", "high"]
    seam_regression_results: list[RegressionRecord]
    coverage_note: str
    domain_layer_leakage: list[str]          # patterns that recurred but are domain-specific, kept OUT of the set
```

**System prompt** (`prompts/system.md`):

Long-form prompt walking the playbook's §3–§9 sections. Loaded as a single H1 stretch + appended sub-sections per playbook step. The prompt is explicit about hard rules from §9:

- Emit only `status: candidate`. Never `standard`.
- Two contracts; never collapse them.
- No TaskType without an applicability decision.
- No dispatchable type without a coherence audit; no archetype without a seam audit at the monolith bar.
- Distinguish application / infrastructure / domain-layer in every output entry.
- Conservative on low evidence: 1–2 occurrences → record, do not invest.
- Never modify source repos. Read + extract only.

**Capability binding:**

Role: `archetype-pattern-extractor`. Capability: `deep-reasoning` (per ADR-037 default escalation tier — the extractor's reasoning is hard; the cost is justified because the output is the catalog).

**Tool surface (per [ADR-070](ADR-070), [ADR-071](ADR-071)):**

- `repo_inspect.list_files(path)` — read-only listing within an instance repo.
- `repo_inspect.read_file(path)` — read-only.
- `ast_walk.parse_rust(path)` / `ast_walk.parse_python(path)` / etc. — language-aware structural walks.
- `candidate_query.read_candidates(archetype_id)` — reads TaskTypeCandidate records from the graph.
- Workspace containment per [FT-124](FT-124) (when shipped) — no escape outside the configured instance repos + the archetype output directory.

**Regression test mode (per playbook §7):**

When `extraction_mode: "regression-test"`, the worker takes a `prior_archetype` + a `known-good instance`, regenerates a sample of its features via dispatch, runs the archetype + seam audits, and emits a `RegressionRecord` per audit. This is the load-bearing safety check that proves the audits have teeth — and is the regression evidence ADR-084 §5 + ADR-085 §1 require for promotion.

**Output writing:**

The worker emits artifacts under `forge/archetypes/{archetype-id}/` (overwriting in `mint` and `amend` modes; read-only in `regression-test` mode). All emitted artifacts are `status: candidate`. The graph-resident archetype + contracts + TaskTypes are NOT written by the worker — the dispatcher's post-worker stage ingests the output directory and writes the artifacts via the typed GraphWriter paths.

**Hand-back report:**

Emit `forge/archetypes/{archetype-id}/EXTRACTION-REPORT.md` per playbook §8: archetype identified, two contracts, instances + variance, census, minted/rejected, layer split, contract-split health, seam-audit status against the monolith bar, coverage estimate, niche signal.

**Test coverage:**

- Positive (mint mode, two synthetic instance repos): worker emits archetype.yaml + contracts + ≥1 TaskType + ≥1 SeamAudit (per ADR-084 §3 required families) + EXTRACTION-REPORT.md. Asserts: all emitted statuses are `candidate`; seam_audits non-empty.
- Positive (amend mode): a prior archetype + a TaskTypeCandidate with `recurrence_hint: LikelyRecurring` → output proposes a new TaskType that addresses the candidate's signature.
- Positive (regression-test mode): synthetic known-good instance + prior archetype → regression records emitted for each seam audit.
- Negative (no seam audits emitted): worker output missing the three required seam-audit families → integration test fails; worker is required to emit them.
- Negative (one instance, low evidence): the worker emits the archetype but flags `instance_variance: high` and `archetype_layer_estimate: 0.0` and prominent caveats in coverage_note.
- Negative (forced near-miss): worker is given a candidate that does not recur ≥3 times → does NOT propose a new TaskType; logs as `rejected_patterns`.
- Application-contract-invariance regression: synthetic instances disagree on language → worker emits `application_contract_held_invariant: false` and flags the archetype boundary as wrong in EXTRACTION-REPORT.md.

### State

- **New on-disk:** `workers/archetype-pattern-extractor/` package; tooling under `src/archetype_pattern_extractor/tools/`.
- **Modified on-disk:** role catalog seed for the new role + capability binding.
- **Output on-disk:** `forge/archetypes/{archetype-id}/` directory structure per playbook §1.

### Behaviour

1. **Dispatch via the broad-worker authority surface, NOT a cell cluster.** The worker is broad-authority by construction — its task type is "explorer-and-typifier", which is exactly what the broad worker covers under [ADR-080](ADR-080).
2. **Capability binding seed updates** for the new role.
3. **Read-only tool surface enforced**. The agentic loop's tools cannot mutate source repos.
4. **All emitted statuses are `candidate`**. No `standard` minting under any circumstances.
5. **Regression test mode is a first-class flow**. Used to provide ADR-084 §5 / ADR-085 §1 evidence.

### Invariants

- **Never `status: standard`.** Hard rule from playbook §9.1 + ADR-085.
- **Seam audits non-empty.** Hard rule from ADR-084. Worker is required to emit ≥1 per required family.
- **Read-only on source repos.** Hard rule from playbook §9.8.
- **Conservative on low evidence.** Hard rule from playbook §9.7.
- **Layer split honest.** Application / infrastructure / domain-layer counts explicit in the EXTRACTION-REPORT.

### Error handling

- **Tool surface violation** (worker attempts to write outside the archetype output directory) → workspace containment guard kills the call; dispatch fails; logged as a tool-safety violation per FT-124.
- **Seam audits missing from output** → integration check at the dispatcher's post-worker stage refuses to write the archetype with E102; worker output discarded; surfaces in dispatch outcome.
- **Regression test mode with no known-good instance** → input validation rejects; worker not dispatched.
- **LiteLLM error (deep-reasoning capability unavailable)** → escalation per ADR-034; if no fallback, dispatch fails.

### Boundaries

- **In scope.** The worker package; role catalog seed; tool surface; system prompt; six TCs across mint / amend / regression-test modes.
- **Out of scope.** The dispatcher's post-worker ingestion stage that writes the typed Archetype + Contracts + TaskTypes to the graph — separate slice (covered by FT-157's planner consuming this worker's output). LLM-judged audit-monolith-bar evaluation — the worker outputs `candidate-audit-weak` by default; human reviewer evaluates evidence and bumps to `passes`. Auto-promotion of any artifact to `standard` — forbidden. Multi-archetype extraction (one run producing two archetypes) — out of v1.

## Out of scope

- Dispatcher post-worker ingestion — FT-157.
- LLM-judged monolith-bar evaluation — human review required.
- Auto-promotion to `standard` — forbidden.
- Multi-archetype extraction.
- Cross-language repo support beyond what `ast_walk` ships in v1.
