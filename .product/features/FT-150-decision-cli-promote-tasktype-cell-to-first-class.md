---
id: FT-150
title: 'decision-cli: Promote TaskType + Cell to first-class artifact types with family/conforms_to/derived_from-contract fields'
phase: 5
status: planned
depends-on:
- FT-147
- FT-148
- FT-149
adrs:
- ADR-082
- ADR-080
tests: []
domains:
- api
- data-model
domains-acknowledged: {}
---

## Description

The fourth substrate feature for [ADR-082](ADR-082). Promotes the TaskType + Cell substrate from feature_spec body convention (the v1 shape from [FT-139](FT-139), [ADR-080](ADR-080) §Rejected §3 deferred) to first-class graph-resident artifact types: `dec:TaskType` and `dec:Cell`. Adds the archetype-layer fields from ADR-082 §3: `family`, `conforms_to`, `derived_from-contract`, `archetype`.

This is the bootstrap-closing slice ADR-080 explicitly deferred. Under FT-139 the TaskType + Cell ontology lived as Markdown bodies under the `FT-TT-<name>` convention; this slice migrates the six existing TaskTypes (FT-139..FT-144) to first-class artifacts and ships the field set the archetype layer needs.

The new fields enable: (a) routing the dispatcher per family (application = pure generation; infrastructure = ordered, side-effecting per [FT-151](FT-151)); (b) propagating `not-safely-dispatchable` flags from weak-audit conventions ([FT-148](FT-148)); (c) checking that every cell's `derived_from-contract` reference resolves to an actual contract convention (the upstream-cell coherence claim from ADR-082 §3).

## Functional Specification

### Inputs

- The six existing TaskType feature_specs ([FT-139](FT-139), [FT-140](FT-140), [FT-141](FT-141), [FT-142](FT-142), [FT-143](FT-143), [FT-144](FT-144)) — the migration source.
- The TaskType + Cell Rust types from `crates/decision-cli/src/core/task_type/` (the FT-139 substrate) — these become the typed-artifact shape.
- The static TaskType registry from FT-139 — replaced by graph-resident lookup post-migration.
- `Archetype` from [FT-147](FT-147), `ApplicationContract` + `Convention` from [FT-148](FT-148), `InfrastructureContractTemplate` from [FT-149](FT-149).

### Outputs

**Rust structs** (`crates/decision-cli/src/core/ontology/task_type.rs` — replaces the FT-139 substrate's `TaskTypeDecl`):

```rust
pub struct TaskType {
    pub id: NamedNode,
    pub name: String,                            // e.g. "add-judge-worker"
    pub archetype: NamedNode,                    // back-reference to the owning Archetype
    pub family: TaskTypeFamily,                  // Application | Infrastructure
    pub conforms_to: Vec<NamedNode>,             // → Convention IRIs (from ApplicationContract or InfrastructureContract conventions)
    pub cells: Vec<NamedNode>,                   // → Cell IRIs
    pub coherence_audit: NamedNode,              // → CoherenceAudit (FT-139's existing type, lifted to artifact)
    pub applicability: Applicability,            // "applies when" + "does NOT apply" clauses (per playbook §5a)
    pub safely_dispatchable: bool,               // computed at registration time from conforms_to.checkable propagation
    pub provenance: Provenance,
}

pub enum TaskTypeFamily { Application, Infrastructure }

pub struct Applicability {
    pub applies_when: String,                    // human + machine-checkable phrasing
    pub does_not_apply: Vec<String>,             // negative clauses
    pub parameters: Vec<ApplicabilityParameter>, // what each parameter switches on
}

pub struct ApplicabilityParameter {
    pub name: String,
    pub switches_on: String,                     // the decision the parameter encodes
}

pub struct Cell {
    pub id: NamedNode,
    pub name: String,
    pub task_type: NamedNode,                    // back-reference
    pub artifact_type: String,                   // logical artifact this cell emits
    pub prompt_template_path: PathBuf,
    pub model_binding_capability_iri: NamedNode,
    pub derived_from_cells: Vec<NamedNode>,      // → other Cell IRIs (intra-cluster ordering)
    pub derived_from_contract: Vec<ContractReference>, // → ApplicationContract or InfrastructureContract conventions
    pub provenance: Provenance,
}

pub struct ContractReference {
    pub contract_kind: ContractKind,             // ApplicationContract | InfrastructureContract
    pub convention_id: NamedNode,
}

pub enum ContractKind { ApplicationContract, InfrastructureContract }
```

**SHACL shapes** (`shapes/task_type.shacl.ttl`):

- `dec:TaskTypeShape sh:targetClass dec:TaskType` with required `name`, `archetype`, `family`, `cells: sh:minCount 1`, `coherence_audit`, `applicability`.
- `dec:CellShape sh:targetClass dec:Cell` with required `name`, `task_type`, `artifact_type`, `prompt_template_path`, `model_binding_capability_iri`.
- Cross-cutting constraint: **every `derived_from_contract` reference resolves to a Convention in the task_type's archetype's contracts** — E112 (`E112_CellContractReferenceUnresolved`).
- Cross-cutting constraint: **`family: infrastructure` requires the cell's `derived_from_contract` to reference at least one InfrastructureContract convention** (consistency check between TaskType family and Cell contract derivation).
- Applicability is required and non-empty for both `applies_when` and at least zero `does_not_apply` (zero is allowed but flagged with W105 — pure-pattern-match TaskTypes are rare).

**Cluster registry → graph-resident lookup:** the static `lazy_static` TaskType registry from FT-139 is replaced by a query-driven lookup. The dispatcher at FT-139's `cluster_dispatch::run` is updated to read TaskType by name from the graph rather than from the static registry. The static registry stays as a startup-time bootstrap loader: it reads the six existing TaskType feature_specs at startup, writes them to the graph as first-class TaskType artifacts (idempotent — only if not already present), then defers all lookups to the graph from that point on.

**Safely-dispatchable propagation:** at TaskType registration / amendment time:
- Walk `conforms_to` → for each Convention, read its `checkable` field.
- If any referenced Convention has `checkable: false`, set `safely_dispatchable: false` on the TaskType.
- Otherwise true.
- The dispatcher refuses to dispatch a TaskType with `safely_dispatchable: false` and routes to the broad-worker escape hatch instead (per [ADR-082](ADR-082) §4, ADR-084 §2).

**Migration of FT-139..FT-144:** for each of the six existing TaskType feature_specs:
1. Read the body's cell declarations, coherence audit, applicability.
2. Construct the typed TaskType + Cell artifacts.
3. Set `family: Application` (all six are application-family in their current shape).
4. Set `archetype: <decision-cli archetype IRI>` (the first archetype lands in [FT-160](FT-160) — for the migration window before that, set to a placeholder `dec:archetype:decision-cli-self-implementation` IRI that FT-160 ratifies).
5. Set `conforms_to: []` initially — FT-160 backfills the convention references when the application contract lands.
6. Write each to the graph.

**Round-trip tests + cluster audit teeth + applicability resolution test:**

- Positive round-trip TaskType + Cell.
- Negative: Cell with unresolved `derived_from_contract` → E112.
- Negative: TaskType with `family: infrastructure` and no InfrastructureContract derived_from → cross-family violation.
- Migration test: invoke the bootstrap loader on a fresh store with the six FT-139..FT-144 specs present → all six TaskType + their Cell sets land in the graph.
- Safely-dispatchable propagation: a TaskType with `conforms_to` pointing at a Convention with `checkable: false` ends up with `safely_dispatchable: false`.

### State

- **New on-disk:** `task_type.rs` (replaces the prior thin module), sub-module `task_type/{parser,emitter,tests}.rs`, `shapes/task_type.shacl.ttl`, `vocab/task_type.rs`, bootstrap loader at `core/task_type/bootstrap.rs`.
- **Modified on-disk:** `features/drive/cluster_dispatch.rs` — replace the static registry lookup with the graph query (with the same shape so callers don't see the change). `core/graph/writer.rs` — typed write methods for TaskType + Cell.
- **Graph-resident:** post-migration, six TaskType + their Cell sets live in the orchestration store.

### Behaviour

1. **Cluster dispatch via `add-artifact-type`**. Two artifact types in one slice (TaskType + Cell). The cluster runs twice. Coherence audit teeth from FT-141.
2. **SHACL chokepoint registration**. New shapes registered; E112 fires on Cell writes with unresolved contract references.
3. **Bootstrap migration runs once per store**. The bootstrap loader is invoked at `dec init` / `dec drive` startup; idempotent — checks for the presence of each TaskType IRI before writing. Existing stores migrate transparently on next startup.
4. **Dispatcher updated to query graph**. `cluster_dispatch::run(workdir, ctx, args, task_type_name)` now reads the TaskType by name from the graph. Behaviour preserved; the source of truth shifts.
5. **Safely-dispatchable propagation runs at TaskType write + at Convention checkable mutation**. A Convention going from `checkable: true → false` triggers a re-computation across all TaskTypes referencing it via `conforms_to`.

### Invariants

- **Every Cell's `derived_from_contract` references resolve.** E112.
- **TaskType `family` and Cell contract derivation are consistent.** Application-family TaskTypes have only ApplicationContract derived_from; infrastructure-family TaskTypes derive at least one InfrastructureContract reference.
- **Static registry is gone post-migration.** The dispatcher queries the graph; if the bootstrap migration has not run, the dispatcher refuses with a startup error.
- **Migration is idempotent.** Re-running the bootstrap loader against a store with the TaskTypes already present is a no-op.
- **`safely_dispatchable` is computed, never authored.** The field is read-only from outside the GraphWriter; mutation routes through Convention-checkable-flip and TaskType-registration paths only.

### Error handling

- **E112** — Cell `derived_from_contract` references unresolved Convention IRI.
- **E113** — TaskType family / Cell contract derivation inconsistency (`family: application` but a Cell derives from InfrastructureContract).
- **Migration failure** — partial migration aborts and rolls back; the store ends in pre-migration state; the dispatcher refuses subsequent dispatches until migration succeeds.
- **Safely-dispatchable check on weak-audit propagation** — re-computation failure (e.g., Convention IRI not found) → log warning + leave TaskType.safely_dispatchable unchanged for safety (conservative).

### Boundaries

- **In scope.** TaskType + Cell artifact types with archetype-layer fields (`family`, `conforms_to`, `derived_from-contract`, `archetype`). Migration of FT-139..FT-144 via bootstrap loader. SHACL shapes + E112 / E113. Safely-dispatchable propagation logic. Dispatcher updated to read TaskType from graph. Six exit-criteria TCs: round-trip TaskType, round-trip Cell, unresolved-contract-ref rejection, family-consistency rejection, migration end-to-end, safely-dispatchable propagation.
- **Out of scope.** Infrastructure TaskType ordering fields — FT-151. The first archetype's actual contract references (FT-160 backfills `conforms_to` and the placeholder archetype IRI). The classifier worker (FT-155). The dispatch planner (FT-157). Migrating the `add-judge-worker` audit from `scripts/checks/` into a proper CoherenceAudit artifact — separate slice. LLM-driven TaskType authoring — a future TaskType candidate.

## Out of scope

- Infrastructure TaskType ordering fields — FT-151.
- First archetype's concrete `conforms_to` references — FT-160.
- Promoting CoherenceAudit to a first-class artifact type — separate slice (`add-artifact-type` cluster candidate).
- Classifier / dispatcher planner — FT-155 / FT-157.
- Multi-archetype TaskTypes (one TaskType serving two archetypes) — possible future expansion; v1 binds a TaskType to one archetype.
- TaskType deprecation / versioning — uses ADR-style amendment when needed.
