---
id: FT-154
title: 'decision-cli: Archetype escape hatch — TaskTypeCandidate artifact and unknown-unit routing'
phase: 5
status: planned
depends-on:
- FT-150
adrs:
- ADR-080
- ADR-082
tests: []
domains:
- api
- data-model
domains-acknowledged: {}
---

## Description

Introduces the **archetype escape-hatch**: when a feature request's unit does not cleanly match any TaskType in the archetype, the classifier routes to the broad code-writer (per [ADR-080](ADR-080)) and emits a `dec:TaskTypeCandidate` artifact recording what the unit was, why no existing TaskType matched, and whether it looks like a recurring shape worth promoting to a typed TaskType.

This is the principled standing of the broad worker codified at the archetype layer. The broad code-writer is not a fallback for "I couldn't be bothered to write a TaskType"; it is the documented explorer-and-typifier for the ~20% of work that lives at the archetype's edge (per `briefs/pattern-extraction-playbook-v2.md` and `briefs/feature-authoring-brief.md §6`). The TaskTypeCandidate artifact is what feeds the catalog-growth path — `dec pattern extract` (later slice) reads them and proposes new TaskTypes.

The brief is direct (`feature-authoring-brief.md §6`): "Your most valuable output, when you hit the unknown, is not the code — it is a clean description of a possible new task type."

## Functional Specification

### Inputs

- The classifier branch from [FT-139](FT-139) — extended to read TaskType applicability (post-[FT-150](FT-150)).
- The broad code-writer ([FT-123](FT-123)) — the dispatch target.
- `Archetype` + `TaskType` from FT-147 / FT-150 — looked up at classify time.
- The `add-artifact-type` TaskType ([FT-141](FT-141)) — implementation cluster for the TaskTypeCandidate type.

### Outputs

**Rust struct** (`crates/decision-cli/src/core/ontology/task_type_candidate.rs`):

```rust
pub struct TaskTypeCandidate {
    pub id: NamedNode,
    pub archetype: NamedNode,                      // the archetype whose edge this candidate is at
    pub source_feature: NamedNode,                 // → the Feature whose unit was unmatched
    pub unit_description: String,                  // what the unit was
    pub reason: UnmatchReason,                     // why no TaskType matched
    pub candidate_signature: Option<String>,       // what a future "applies when" might say
    pub recurrence_hint: RecurrenceHint,           // FirstSeen | SeenBefore | LikelyRecurring
    pub broad_worker_session: NamedNode,           // → the SessionRecord that handled the unit
    pub contract_pressure: Vec<ContractPressure>,  // signals that the archetype's contracts may need extension
    pub provenance: Provenance,
}

pub enum UnmatchReason {
    NoApplicableType,                              // no TaskType's "applies when" matched
    DoesNotApplyClauseFired { task_type, clause },
    AmbiguousBetweenTypes { types: Vec<NamedNode> },
    RequiresDomainComputation,                     // explicit domain-layer signal
}

pub enum RecurrenceHint { FirstSeen, SeenBefore, LikelyRecurring }

pub struct ContractPressure {
    pub kind: ContractPressureKind,                // ApplicationContractNeeds | InfrastructureContractNeeds
    pub detail: String,
}

pub enum ContractPressureKind {
    ApplicationContractNeeds,                      // a cross-cutting convention seems needed
    InfrastructureContractNeeds,                   // a new resource class seems needed
    NewArchetypeBoundary,                          // the unit doesn't fit this archetype at all
}
```

**SHACL shape** (`shapes/task_type_candidate.shacl.ttl`):

- Required `archetype`, `source_feature`, `unit_description`, `reason`, `broad_worker_session`.
- `RecurrenceHint::LikelyRecurring` requires `candidate_signature: sh:minCount 1` (a likely-recurring candidate without a signature is useless to the extractor).

**Classifier extension at `features/drive/planners/feature_ship.rs`:**

Pre-FT-154 classifier branch (FT-139 + FT-150): match against TaskType applicability; if matched, dispatch the cluster; if not, dispatch the broad worker.

Post-FT-154: when the broad-worker dispatch path is taken, additionally:
1. Record an `UnmatchReason` based on why no TaskType matched (which of the four enum variants applies).
2. Compute a `RecurrenceHint` by querying the graph: count prior TaskTypeCandidate records with similar `unit_description` (cosine similarity over BM25, or simple substring grouping for v1) — count ≥3 → `LikelyRecurring`; 1–2 → `SeenBefore`; 0 → `FirstSeen`.
3. After the broad worker's session completes, create a TaskTypeCandidate linking source_feature, broad_worker_session, the reason, and the recurrence_hint.
4. If broad worker emits any contract-pressure signals (via worker feedback per [FT-031](FT-031)), populate `contract_pressure`.

**`dec pattern extract` candidate surfacing** (later slice — FT-156 covers it):

The pattern-extractor reads TaskTypeCandidate records with `recurrence_hint: LikelyRecurring`. When ≥3 candidates with similar signatures exist, the extractor proposes minting a new TaskType. The proposal is human-reviewed (mirrors ADR-085's promotion gating); auto-promotion is forbidden.

**No-force-match invariant:**

The classifier is forbidden from forcing a near-miss match. If `applies_when` matches but a `does_not_apply` clause fires, route to escape hatch — do not dispatch the cluster. This is the playbook's hard rule (`§7.4`, `§9.3`): "A near-miss dispatch is worse than an honest escalation." Encoded as a structural check in the classifier: any TaskType match where any `does_not_apply` clause matches is downgraded from `Match { confidence: High }` to `NoMatch { reason: DoesNotApplyClauseFired { task_type, clause } }`.

**Test coverage:**

- Positive: feature unit matches a TaskType → cluster dispatches (no TaskTypeCandidate created).
- Negative (escape-hatch path): feature unit matches no TaskType → broad worker dispatches; TaskTypeCandidate created with `UnmatchReason::NoApplicableType`.
- Negative (does-not-apply clause fires): feature unit's `applies_when` matches but a `does_not_apply` clause matches too → broad worker dispatches; TaskTypeCandidate with `DoesNotApplyClauseFired`.
- Negative (ambiguous match): unit matches two TaskTypes → broad worker dispatches; TaskTypeCandidate with `AmbiguousBetweenTypes`.
- Recurrence hint computation: with 0 prior similar candidates → `FirstSeen`; with 4 → `LikelyRecurring`.
- Contract-pressure capture: broad worker emits a feedback signal "this would benefit from a cross-cutting auth convention" → ContractPressure with `ApplicationContractNeeds`.
- SHACL: `RecurrenceHint::LikelyRecurring` without `candidate_signature` → rejected.
- No-force-match invariant: classifier with a TaskType whose `applies_when` matches AND `does_not_apply` matches → NoMatch with `DoesNotApplyClauseFired` — not Match.

### State

- **New on-disk:** `task_type_candidate.rs`, sub-module `task_type_candidate/{parser,emitter,tests}.rs`, `shapes/task_type_candidate.shacl.ttl`, `vocab/task_type_candidate.rs`.
- **Modified on-disk:** `features/drive/planners/feature_ship.rs` (no-force-match check + candidate emission), `features/drive/cluster_dispatch.rs` (broad-worker dispatch path emits candidate post-session).
- **No orchestration-store schema change beyond the new type.**

### Behaviour

1. **Cluster dispatch via `add-artifact-type`**. One artifact type. FT-141 audit teeth.
2. **Classifier check enforces no-force-match**. The check runs at every classification call.
3. **Escape-hatch emission**. Every time the broad worker is dispatched as the classifier's chosen path, a TaskTypeCandidate is created.
4. **Recurrence-hint scoring**. Reads prior candidates from the graph; deterministic similarity scoring.
5. **Contract-pressure capture**. Worker feedback consumed and recorded.

### Invariants

- **The broad worker is the only escape hatch.** Other code paths cannot create TaskTypeCandidates — only the classifier-to-broad-worker route.
- **Every escape-hatch dispatch creates a candidate.** No silent broad-worker dispatches; the catalog-growth path has visibility into every unmatched unit.
- **No forced near-miss matches.** Structural check: any TaskType whose `does_not_apply` clause matches the unit is downgraded to NoMatch.
- **`LikelyRecurring` requires a signature.** SHACL.
- **Candidates are immutable evidence.** Once written, the source_feature, reason, and broad_worker_session do not change. Recurrence hint may update on re-classification of the same source unit (rare; usually a re-author).

### Error handling

- **SHACL rejection (`LikelyRecurring` without signature)** → write refused at GraphWriter chokepoint.
- **Broad worker session creation failure** → cluster dispatch fails normally; no orphan candidate created.
- **Recurrence scorer unavailable (e.g., similarity index broken)** → fall back to `FirstSeen`; log a warning.
- **Contract-pressure parser failure on worker feedback** → ignore the feedback; log a warning; candidate emitted with empty contract_pressure.

### Boundaries

- **In scope.** TaskTypeCandidate artifact type; SHACL shape + the LikelyRecurring/signature constraint; classifier no-force-match enforcement; escape-hatch emission; recurrence-hint scoring (v1: simple substring grouping); contract-pressure capture; eight TCs.
- **Out of scope.** The pattern-extractor that consumes candidates — FT-156. Authoring new TaskTypes from candidates — that is the FT-156 + human-review path; this slice ships the input data. LLM-driven similarity scoring — v1 uses substring grouping. Workers self-proposing TaskTypes — strictly broad-worker-only for v1. Multi-archetype candidates (a candidate that might fit a different archetype) — possible future expansion; v1 binds candidate to the source archetype.

## Out of scope

- The pattern-extractor — FT-156.
- LLM-driven similarity scoring — v1 uses substring grouping.
- Self-proposing workers (non-broad workers minting candidates).
- Multi-archetype candidates.
- Auto-promotion of candidates to TaskTypes — never; always human-gated (per ADR-085's analogue at the TaskType layer).
