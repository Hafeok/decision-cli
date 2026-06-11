---
id: FT-149
title: 'decision-cli: InfrastructureContract template + per-instance binding artifact types'
phase: 5
status: planned
depends-on:
- FT-147
- FT-148
- FT-167
adrs:
- ADR-082
- ADR-083
tests: []
domains:
- api
- data-model
domains-acknowledged: {}
---

## Description

The third substrate feature for [ADR-082](ADR-082). Introduces `dec:InfrastructureContractTemplate` (archetype-bound — declares slots) and `dec:InfrastructureContractInstance` (per-customer — fills slots, frozen at Discovery) as the parallel contract surface to [FT-148](FT-148)'s ApplicationContract.

The template owns the slot specification: compute, data engine, identity provider, secrets, messaging, observability. Each slot declares its required shape (an audit can check what fills it) and the **satisfaction rule** linking it back to the application contract — e.g., the data-engine slot must satisfy ApplicationContract.persistence_model. The instance owns the concrete choices for one customer; once written, mutation requires explicit re-contracting.

This is the IaC cell from `briefs/system-archetype-spec-v2.md §5` made first-class: it runs parallel to the application contract, owns its own domain decisions, and feeds a distinct family of TaskTypes (infrastructure-family, [FT-150](FT-150) + [FT-151](FT-151)).

## Functional Specification

### Inputs

- `Archetype` from [FT-147](FT-147) — links to one template + many instances.
- `ApplicationContract` from [FT-148](FT-148) — slots in the infrastructure contract declare satisfaction rules against application contract conventions.
- The `add-artifact-type` TaskType ([FT-141](FT-141)) — implementation cluster.
- The IaC outputs declared on the instance feed seam audits (FT-152, FT-153) — the seam audit consumes the instance's output set.

### Outputs

**Rust structs** (`crates/dec-ontology/src/ontology/infrastructure_contract.rs`):

```rust
pub struct InfrastructureContractTemplate {
    pub id: NamedNode,
    pub archetype: NamedNode,
    pub slots: Vec<InfraSlot>,                  // compute, data-engine, identity, secrets, messaging, observability
    pub provenance: Provenance,
}

pub struct InfraSlot {
    pub id: NamedNode,
    pub name: String,                            // "compute", "data-engine", etc.
    pub required: bool,
    pub satisfaction_rule: SatisfactionRule,     // links to an ApplicationContract convention this slot satisfies
    pub legal_choices: Vec<String>,              // e.g. ["azure-sql", "postgres-flexible"] — the constraint the instance fills against
    pub iac_outputs: Vec<String>,                // the names of outputs an instance MUST emit (e.g. "connection-string", "endpoint", "managed-identity-client-id")
}

pub struct SatisfactionRule {
    pub application_contract_convention_id: NamedNode,
    pub assertion: String,                        // human-readable + machine-checkable phrasing (e.g. "must satisfy SQL-domain-model")
}

pub struct InfrastructureContractInstance {
    pub id: NamedNode,
    pub archetype: NamedNode,
    pub template: NamedNode,                      // → InfrastructureContractTemplate
    pub customer_id: String,                      // free-form identifier for the customer / environment
    pub status: InstanceStatus,                   // Draft | Frozen (frozen = mutation requires re-contracting)
    pub slot_choices: Vec<SlotChoice>,            // one per template slot
    pub iac_outputs: Vec<IaCOutput>,              // concrete outputs the instance emits — feeds seam audits
    pub satisfaction_record: Vec<SatisfactionEvidence>, // per-slot: did the choice satisfy the application contract?
    pub frozen_at: Option<DateTime>,
    pub frozen_by: Option<String>,
    pub provenance: Provenance,
}

pub struct SlotChoice {
    pub slot_id: NamedNode,
    pub value: String,                            // one of the slot's legal_choices
    pub satisfaction_evidence: String,            // free-text describing how the choice satisfies the satisfaction_rule
}

pub struct IaCOutput {
    pub name: String,                             // matches a slot's iac_outputs entry
    pub value_shape: String,                      // "url", "secret-ref", "json-blob", "managed-identity-client-id"
    pub source_module: String,                    // e.g. "azure-keyvault-module" — the Bicep module that emits it
}

pub struct SatisfactionEvidence {
    pub slot_id: NamedNode,
    pub satisfied: bool,
    pub note: String,
}

pub enum InstanceStatus { Draft, Frozen }
```

**SHACL shapes** (`shapes/infrastructure_contract.shacl.ttl`):

- `InfrastructureContractTemplateShape sh:targetClass dec:InfrastructureContractTemplate` with `sh:minCount 1` on `slots`.
- `InfrastructureContractInstanceShape sh:targetClass dec:InfrastructureContractInstance` with `sh:minCount 1` on `template`, `customer_id`, `status`, and a constraint that **`status: frozen` requires `frozen_at` and `frozen_by`**.
- Cross-shape constraint via SHACL `sh:and`: **every slot in the template has a corresponding `slot_choices` entry in the instance** — E108 (`E108_InfrastructureInstanceMissingSlotChoice`).
- **Mutation guard:** updates to an instance with `status: frozen` must go through the explicit re-contracting CLI path (`dec archetype reframe-instance ...`) — any other write path is refused with E020-style enforcement.

**IRI vocabulary, parser, emitter, round-trip tests:** as per FT-141 cluster.

**Test coverage:**

- Positive: build a template with three slots, build an instance filling all three with one IaCOutput per slot, freeze the instance, round-trip, assert equality.
- Negative (instance missing a slot choice for a required template slot) → E108.
- Negative (instance frozen but `frozen_at` absent) → E109 (`E109_InfrastructureInstanceFrozenMissingTimestamp`).
- Negative (mutation of a frozen instance via non-CLI path) → E020.
- Positive (re-contracting via CLI path): allowed mutation flips Draft from Frozen with audit record.

### State

- **New on-disk:** `infrastructure_contract.rs`, sub-module `infrastructure_contract/{parser,emitter,tests}.rs`, `shapes/infrastructure_contract.shacl.ttl`, `vocab/infrastructure_contract.rs`.
- **Modified on-disk:** ontology re-exports; SHACL shape registration; GraphWriter typed methods for template + instance.
- **Conventions on-disk path:** the infrastructure contract conventions (naming, networking, identity per spec §3) live under `forge/archetypes/{id}/infrastructure/conventions/{name}.md`. Bodies are referenced by ID; loaded same way as application conventions.

### Behaviour

1. **Cluster dispatch via `add-artifact-type`**. Two artifact types in one slice (template + instance). The cluster runs twice — once per artifact type. The coherence audit passes both runs independently.
2. **Satisfaction rule check at instance write.** When an instance is written, the SHACL chokepoint also runs a side-table check: for each slot_choice, the corresponding slot's satisfaction_rule must be satisfied — i.e., the choice's value must appear in the slot's `legal_choices` *and* the slot's satisfaction_rule's assertion must be marked `satisfied: true` in the instance's satisfaction_record. Failure → E111 (`E111_InfrastructureInstanceSatisfactionUnproven`).
3. **Freeze gate.** An instance can transition `Draft → Frozen` via `GraphWriter::freeze_instance(...)` which is itself only callable from the CLI freeze path. Once frozen, every mutation refuses with E020 unless routed through the explicit reframe-instance command.
4. **IaC output registry.** The instance's `iac_outputs` list is queryable as a typed table — feeds the seam-audit runner (FT-153 / FT-160).

### Invariants

- **Template slot count is the floor for instance slot_choices.** Every required slot has a matching choice. SHACL E108.
- **Frozen instances are immutable except via reframe-instance.** E020 / freeze gate.
- **Satisfaction is structural, not aspirational.** Every slot_choice carries `satisfaction_evidence`; the instance's `satisfaction_record` carries a `satisfied: bool` per slot. Both must align with the slot's `satisfaction_rule` for the SHACL write to succeed.
- **IaC outputs declared up front.** An instance cannot "discover" new outputs at dispatch time; the iac_outputs field is the complete output set. New outputs require a re-contracting amendment.

### Error handling

- **E108** — instance missing slot choice for required slot.
- **E109** — instance status frozen but frozen_at / frozen_by absent.
- **E111** — slot_choice value does not satisfy the slot's satisfaction_rule.
- **E020** — mutation of frozen instance outside the reframe-instance CLI path.
- **Cluster audit failure during `add-artifact-type` dispatch** → standard FT-139 rollback.

### Boundaries

- **In scope.** Both artifact types (template + instance), their SHACL shapes, IRI vocab, parser, emitter, round-trip tests; the freeze gate; satisfaction-rule enforcement at write time; E108 / E109 / E111 / E020. Six exit-criteria TCs: template round-trip, instance round-trip + freeze, missing-slot-choice rejection, frozen-no-timestamp rejection, unproven-satisfaction rejection, frozen-mutation rejection via CLI bypass.
- **Out of scope.** The reframe-instance CLI path itself (lands in FT-158 alongside the other `dec archetype` verbs). Authoring the first concrete template + instance (FT-160). TaskType family + infrastructure-family ordering fields (FT-151). SeamAudit consumption of the iac_outputs registry (FT-152 / FT-153). Cross-archetype shared infrastructure contracts (e.g. a customer with two archetypes using the same Key Vault) — out of v1; modelled in a later slice.

## Out of scope

- Template + instance for the first archetype — FT-160.
- Reframe-instance CLI command — FT-158.
- TaskType family / ordering fields — FT-150, FT-151.
- SeamAudit consumption — FT-152, FT-153.
- Multi-archetype shared infrastructure contracts.
- LLM-driven contract template authoring.
