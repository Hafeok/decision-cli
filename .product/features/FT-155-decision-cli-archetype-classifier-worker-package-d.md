---
id: FT-155
title: 'decision-cli: archetype-classifier worker package — decompose feature request into units and match each against TaskType applicability'
phase: 5
status: planned
depends-on:
- FT-150
- FT-154
adrs:
- ADR-082
tests: []
domains:
- api
domains-acknowledged: {}
---

## Description

A Python judge-style worker that decomposes a feature request into typed units and classifies each unit against the archetype's TaskType applicability set. Implements the **CLASSIFY** step of the dispatch loop from `briefs/feature-authoring-brief.md §1.1`.

Today (post-FT-139..FT-150), classification is operator-declared: the feature_spec carries `task_type: <name>` in its front-matter and the classifier dispatches that cluster. That works for features authored with the catalog in hand but does not scale — the spec author has to know the catalog before the unit can be matched. The archetype-classifier worker reads a free-form feature request, identifies its constituent units, and emits per-unit classification verdicts: a TaskType IRI with confidence, or an explicit escape-hatch verdict ([FT-154](FT-154)).

The worker rides the `add-judge-worker` TaskType ([FT-139](FT-139)) — judge-shape clusters are the established pattern (5 shipped, FT-127/132/133/...).

## Functional Specification

### Inputs

- A `Bundle` containing the feature request text, the target archetype, and the archetype's TaskType set with applicability records.
- LiteLLM proxy via the worker SDK's `LiteLLMClient` ([FT-081](FT-081)), capability tag from the role catalog ([FT-121](FT-121)).
- TaskType applicability fields (post-[FT-150](FT-150)): `applies_when`, `does_not_apply`, parameters.
- The pyoxigraph in-memory store from [FT-049](FT-049) — the worker queries the bundle, not the production graph.

### Outputs

**Python worker package** at `workers/archetype-classifier/`:

```
workers/archetype-classifier/
├── pyproject.toml                          # uv-managed; depends on _shared + worker-sdk + instructor
├── src/archetype_classifier/
│   ├── __init__.py
│   ├── models.py                           # Pydantic ClassificationInput / ClassificationOutput
│   ├── prompts/system.md                   # judge system prompt
│   ├── agent/loop.py                       # LiteLLM call loop
│   └── main.py                             # SSE consumer + dispatch wiring
└── tests/test_classifier.py
```

**Pydantic IO** (`models.py`):

```python
class FeatureUnit(BaseModel):
    description: str
    role_hint: Literal["frontend", "backend", "domain", "infrastructure", "tests", "config"]

class ClassificationInput(BaseModel):
    feature_id: str
    feature_request: str                     # free-form text
    archetype_id: str
    task_types: list[TaskTypeRecord]         # each carries applies_when + does_not_apply
    application_contract_summary: str
    infrastructure_contract_summary: str | None

class ClassificationVerdict(BaseModel):
    unit: FeatureUnit
    match: Literal["high-confidence", "low-confidence", "no-match"]
    matched_task_type: str | None            # IRI; required when match != "no-match"
    confidence_reason: str
    unmatch_reason: Literal[
        "no-applicable-type",
        "does-not-apply-clause-fired",
        "ambiguous-between-types",
        "requires-domain-computation",
    ] | None
    triggered_does_not_apply: str | None     # the clause text
    ambiguous_alternatives: list[str]         # IRIs

class ClassificationOutput(BaseModel):
    feature_id: str
    units: list[ClassificationVerdict]
    contract_pressure: list[ContractPressureSignal]  # see FT-154

class ContractPressureSignal(BaseModel):
    kind: Literal["application-contract-needs", "infrastructure-contract-needs", "new-archetype-boundary"]
    detail: str
```

**System prompt** (`prompts/system.md`):

Stated as a judge: read the feature request, decompose into units; for each unit, walk the TaskType set in priority order; for each TaskType, evaluate `applies_when` + every `does_not_apply` clause; emit a verdict. Hard rules in the prompt: never force a near-miss match; if ambiguous, route to escape hatch with the alternatives listed; if the unit requires domain computation beyond the archetype's TaskType set, route to escape hatch with `requires-domain-computation`.

The prompt cites the playbook's principle directly: "Your most valuable output, when you hit the unknown, is not the code — it is a clean description of a possible new task type."

**Capability binding:**

Role catalog seed (extending [FT-058](FT-058)): a new role `archetype-classifier` with capability `judge-mid-reasoning` (reusing the `add-judge-worker` cluster's standard binding — qwen3-coder via Scaleway, with Anthropic Claude Sonnet escalation for ambiguous units).

**Coherence audit** (`scripts/checks/cluster-audit-archetype-classifier.py`):

Same shape as the FT-127 / FT-132 / FT-133 judge audits — Pydantic models match worker reads, capability binding endpoint + model_id valid, system prompt references field names that exist on the input model, unit-tests fixture validates against input model.

**Dispatch path:**

The dispatcher reads the verdicts:
- All units `high-confidence` → proceed to PLAN with the matched TaskTypes.
- Any unit `low-confidence` or `no-match` → that unit routes to the broad-worker escape hatch ([FT-154](FT-154)); the rest of the cluster proceeds.
- `contract_pressure` populated → surfaced in the cluster report; no dispatch refusal.

**Test coverage:**

- Positive (single unit, high confidence): feature request "add a new audit type for X" → classification matches `add-archetype-audit` (a hypothetical future TaskType); one unit, high-confidence verdict, correct TaskType IRI.
- Positive (multi-unit, mixed confidence): feature request mixes "add an audit" + "add a CLI command" → two units, both high-confidence, two matched TaskTypes.
- Negative (no match): feature request "compute customer churn risk score from billing data" (domain logic) → no-match with `requires-domain-computation`.
- Negative (does-not-apply): feature request matches `applies_when` of TaskType T but T's `does_not_apply` clause fires → no-match with `does-not-apply-clause-fired` and triggered_does_not_apply populated.
- Negative (ambiguous): unit matches two TaskTypes → no-match with `ambiguous-between-types` and alternatives populated.
- Contract pressure: feature request mentions cross-cutting "we should add caching everywhere" → contract_pressure signal with `application-contract-needs`.

### State

- **New on-disk:** `workers/archetype-classifier/` package; `scripts/checks/cluster-audit-archetype-classifier.py`.
- **Modified on-disk:** role catalog seed (new role + capability binding); `workers/_shared/` if any shared helpers added.
- **Graph state:** runtime classification verdicts are not persisted as standalone artifacts in v1 — they live in the dispatch session's outputs and feed FT-157's planner. Persistence as a typed `dec:ClassificationVerdict` artifact is a possible future extension.

### Behaviour

1. **Cluster dispatch via `add-judge-worker`**. Five cells. FT-139 audit teeth.
2. **Capability binding seed updates**. New role registered in role catalog bootstrap.
3. **SSE consumer + dispatch wiring**. Standard worker SDK pattern (FT-077, FT-078).
4. **Verdict consumption by planner**. FT-157 reads the verdicts and decides PLAN.

### Invariants

- **No forced near-miss matches.** The system prompt enforces; the consuming planner enforces too via the FT-154 structural check.
- **Every unit gets a verdict.** No silent drops; if the worker cannot classify a unit, it emits a `no-match` verdict with reason.
- **Contract pressure is advisory.** Never blocks dispatch; surfaces in reports for catalog evolution.
- **Per-unit decisions are independent.** A `no-match` on unit B does not prevent unit A's high-confidence match from dispatching.

### Error handling

- **Worker SDK errors (LiteLLM unreachable, structured-output parse failure)** → bubble up; dispatch fails; broad worker takes over for the whole feature (not per-unit) as a conservative fallback.
- **Bundle missing required fields** → fail at SDK bundle validation; dispatch refuses.
- **Capability binding resolution failure** → `ClusterDispatchError::NoCapabilityForCell` from FT-139.

### Boundaries

- **In scope.** The worker package; the role catalog seed update; the coherence audit; six TCs.
- **Out of scope.** The PLAN step that consumes verdicts — FT-157. Persisting classification verdicts as standalone graph artifacts. LLM-based applicability authoring — humans write applicability fields. Multi-archetype classification (the feature targets two archetypes simultaneously) — out of v1.

## Out of scope

- Planner consumption — FT-157.
- Verdict persistence as standalone artifacts.
- LLM-driven applicability authoring.
- Multi-archetype features.
- Embedding-similarity unit matching (future enhancement).
