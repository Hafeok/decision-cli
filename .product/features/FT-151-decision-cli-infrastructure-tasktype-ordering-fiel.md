---
id: FT-151
title: 'decision-cli: Infrastructure TaskType ordering fields — provisioning.depends_on, idempotency, side_effects'
phase: 5
status: planned
depends-on:
- FT-150
adrs:
- ADR-082
tests: []
domains:
- api
- data-model
domains-acknowledged: {}
---

## Description

Extends the TaskType artifact from [FT-150](FT-150) with the three fields infrastructure-family TaskTypes need under [ADR-082](ADR-082) §3 and the spec's §7: `provisioning.depends_on`, `provisioning.idempotency`, `provisioning.side_effects`. Adds the dispatch-time enforcement of `depends_on` ordering and idempotency declarations.

Application TaskTypes are pure generation: dispatch → audit → done. Infrastructure TaskTypes are not — they touch real cloud state, have ordering against each other (you cannot add a Key Vault secret before the Key Vault exists), and must be idempotent (re-running a declarative Bicep `what-if` is safe; imperative steps need a guard). This slice ships the substrate that records those facts and the planner enforcement that honours them.

Decision-cli has no infrastructure-family TaskType yet — every existing TaskType (FT-139..FT-144) is application-family. This slice ships the field shape, the enforcement logic, and the test coverage; the first infrastructure-family TaskType lands as part of [FT-160](FT-160) (the decision-cli archetype's LiteLLM-proxy provisioning being the witnessed candidate).

## Functional Specification

### Inputs

- `TaskType` from [FT-150](FT-150).
- The dispatcher at `features/drive/cluster_dispatch.rs` (post-FT-139, post-FT-150).
- `InfrastructureContractTemplate` + `InfrastructureContractInstance` from [FT-149](FT-149) — infrastructure TaskTypes derive from instance contents.

### Outputs

**Rust struct extension** (`crates/dec-ontology/src/ontology/task_type.rs`):

```rust
pub struct TaskType {
    // ... existing fields from FT-150 ...
    pub provisioning: Option<ProvisioningPolicy>,  // None for family=Application; Some for family=Infrastructure
}

pub struct ProvisioningPolicy {
    pub depends_on: Vec<NamedNode>,                // → other infrastructure TaskType IRIs
    pub idempotency: Idempotency,
    pub side_effects: bool,                        // always true for infrastructure family
    pub guard_script: Option<PathBuf>,             // required when idempotency=Imperative
}

pub enum Idempotency { Declarative, Imperative }
```

**SHACL shape extension** (`shapes/task_type.shacl.ttl`):

- `family: infrastructure` requires `provisioning: sh:minCount 1`. Cross-shape constraint: E114 (`E114_InfrastructureTaskTypeMissingProvisioning`).
- `family: application` requires `provisioning: sh:maxCount 0`. E115 (`E115_ApplicationTaskTypeHasProvisioning`).
- `idempotency: Imperative` requires `guard_script: sh:minCount 1`. E116 (`E116_ImperativeProvisioningMissingGuard`).
- `side_effects: false` invalid for `family: infrastructure`. E117 (`E117_InfrastructureWithoutSideEffects`).
- `depends_on` cycle detection: forbid cycles in the infrastructure TaskType `depends_on` graph. E118 (`E118_ProvisioningDependencyCycle`).

**Dispatcher enforcement at `features/drive/cluster_dispatch.rs`:**

- Pre-dispatch ordering pass: when an infrastructure TaskType is dispatched, walk its `depends_on` set. For each dependency, verify it has dispatched and audited green against the current InfrastructureContractInstance. If any dependency is unsatisfied, refuse dispatch with `ClusterDispatchError::InfrastructureDependencyUnsatisfied { task_type, missing_dep }`.
- Topological dispatch ordering: when multiple infrastructure TaskTypes are part of one feature's cluster set, dispatch them in `depends_on`-topological order (Kahn's algorithm; cycle is already E118 at write time).
- Idempotency guard at dispatch time: when an infrastructure TaskType is dispatched against an instance where it previously ran successfully, the dispatcher checks:
  - `Idempotency::Declarative` → safe; re-run; Bicep `what-if` reports drift if any.
  - `Idempotency::Imperative` → invoke the `guard_script` first; if exit 0 (safe to re-apply), dispatch; if exit 1 (already applied, do not re-run), skip with `ClusterOutcome::ProvisioningAlreadyApplied { task_type }`; if exit non-zero non-1 → fail with `ClusterDispatchError::IdempotencyGuardFailed { task_type, exit_code }`.
- Application-first ordering refusal: when a feature's cluster set contains both application and infrastructure TaskTypes, dispatch infrastructure-family TaskTypes first. If an application TaskType reads an iac_output that no dispatched-and-audited infrastructure TaskType emits, refuse with `ClusterDispatchError::AppReadsUnprovisionedResource { app_task_type, resource_name }`.

**Test coverage:**

- Positive: build an infrastructure TaskType with declarative idempotency and one declared dependency; round-trip; SHACL passes.
- Negative: build an application TaskType with provisioning → E115.
- Negative: build an infrastructure TaskType without provisioning → E114.
- Negative: imperative idempotency without guard_script → E116.
- Negative: cycle in depends_on → E118.
- Dispatch ordering test: a feature with two infrastructure TaskTypes where B depends_on A; assert A dispatches first.
- Dispatch refusal test: feature with infrastructure TaskType B whose depends_on A has not dispatched; assert refusal.
- Idempotency guard test (Imperative): mock guard exits 1; assert ProvisioningAlreadyApplied outcome.
- Application-first ordering test: feature with one infrastructure provisioning a Key Vault and one application reading a Key Vault secret; assert infrastructure dispatches first; the application's iac_output expectation is met.

### State

- **Modified on-disk:** `task_type.rs` (extension), `shapes/task_type.shacl.ttl` (extension), `features/drive/cluster_dispatch.rs` (ordering + idempotency logic), `vocab/task_type.rs` (new IRI constants for provisioning fields).
- **No new artifact types** — extends existing TaskType.

### Behaviour

1. **Cluster dispatch via `add-artifact-type`?** No — this slice extends an existing artifact type. Lands as a direct implementation (under [ADR-080](ADR-080)'s escape-hatch path), classifier returns no match for `extend-artifact-type` since no such TaskType exists.
2. **SHACL extensions registered**. E114..E118 enforce the family / provisioning consistency at write time.
3. **Dispatcher ordering enforcement runs every dispatch**. Reads the TaskType's `provisioning` block; walks `depends_on`; applies idempotency guard.
4. **Application-first ordering** is implicit in the topological sort — infrastructure TaskTypes have no `depends_on` against application TaskTypes (rejected at SHACL time), so the topological order always places infrastructure before applications.

### Invariants

- **Application TaskTypes have no provisioning.** E115.
- **Infrastructure TaskTypes have provisioning.** E114.
- **Imperative provisioning has a guard.** E116.
- **No cycles in `depends_on`.** E118.
- **Dispatch ordering is topological.** No infrastructure TaskType dispatches before its `depends_on` chain.
- **Application TaskTypes cannot run before their infrastructure dependencies.** Resource-expectation check at dispatch time refuses.

### Error handling

- **E114..E118** at write time (SHACL).
- **`ClusterDispatchError::InfrastructureDependencyUnsatisfied`** at dispatch time.
- **`ClusterDispatchError::IdempotencyGuardFailed`** when imperative guard returns unexpected exit code.
- **`ClusterDispatchError::AppReadsUnprovisionedResource`** when an application TaskType's expected resource is not in any dispatched infrastructure TaskType's iac_outputs.
- **`ClusterOutcome::ProvisioningAlreadyApplied`** is a success outcome, not an error — surfaced in drive history for operator visibility.

### Boundaries

- **In scope.** The ProvisioningPolicy + Idempotency types; SHACL extensions E114..E118; dispatcher ordering + idempotency enforcement; nine test cases.
- **Out of scope.** Authoring the first infrastructure-family TaskType — lands in FT-160 alongside the decision-cli archetype's LiteLLM-proxy provisioning. The seam-audit pipeline (FT-152, FT-153). Bicep templating helpers / declarative-IaC scaffolding — separate slice. Cross-archetype shared infrastructure (a TaskType used by two archetypes' instances) — modelled in a later slice. Dynamic provisioning state (Pulumi-style stack state) — declarative-only for v1.

## Out of scope

- First infrastructure-family TaskType — FT-160.
- Seam-audit consumption — FT-152, FT-153.
- Bicep / IaC template authoring helpers.
- Cross-archetype shared infrastructure resources.
- Stateful provisioning runtime (Pulumi / Terraform state).
