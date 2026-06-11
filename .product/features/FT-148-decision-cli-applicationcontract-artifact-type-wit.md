---
id: FT-148
title: 'decision-cli: ApplicationContract artifact type with checkable-convention schema'
phase: 5
status: planned
depends-on:
- FT-147
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

The second substrate feature for [ADR-082](ADR-082). Introduces `dec:ApplicationContract` as a graph-resident artifact type — the archetype-invariant contract that every application TaskType derives from.

The application contract states, *checkably*, the six required convention items from ADR-082 §2: language/runtime, layering rule, feature organisation, persistence model, endpoint convention, cross-cutting conventions. Each item is a `Convention` sub-artifact with a body precise enough that an audit can mechanically check conformance. A convention that cannot be checked cannot be an audit, and TaskTypes depending on it are not safely dispatchable.

The contract is what makes the catalog economic model real: one archetype serves many customers because the application contract holds invariant while only the infrastructure layer flexes. This slice ships the structure that holds the invariant. The first concrete contract — for the decision-cli self-implementation archetype — lands in [FT-160](FT-160).

## Functional Specification

### Inputs

- The `Archetype` artifact type from [FT-147](FT-147), which links to `application_contract: ApplicationContract`.
- The existing ontology infrastructure: SHACL chokepoint ([ADR-041](ADR-041)), IRI vocabulary modules, dual provenance ([FT-072](FT-072) / [FT-073](FT-073)).
- The `add-artifact-type` TaskType ([FT-141](FT-141)) — the cluster this slice's implementation rides.
- [ADR-083](ADR-083)'s litmus tests + `scripts/checks/tech-detail-binding-level.sh` — this slice ships v1 of the check.

### Outputs

**Rust struct** (`crates/decision-cli/src/core/ontology/application_contract.rs`):

```rust
pub struct ApplicationContract {
    pub id: NamedNode,
    pub archetype: NamedNode,                   // back-reference to the owning Archetype
    pub language_runtime: Convention,           // e.g. "C# / .NET 9"
    pub layering_rule: Convention,              // e.g. "Clean Architecture dependency rule"
    pub feature_organisation: Convention,       // e.g. "vertical slices"
    pub persistence_model: Convention,          // e.g. "SQL domain model + EF Core conventions"
    pub endpoint_convention: Convention,        // e.g. endpoint == contract == frontend-call == test
    pub cross_cutting: Vec<Convention>,         // auth, validation, error handling, logging
    pub provenance: Provenance,
}

pub struct Convention {
    pub id: NamedNode,
    pub name: String,                           // e.g. "slice", "clean-architecture", "persistence"
    pub body_path: PathBuf,                     // forge/archetypes/{id}/application/conventions/{name}.md
    pub audit_id: Option<NamedNode>,            // → ArchetypeAudit IRI that checks this convention
    pub checkable: bool,                        // false → dependent TaskTypes are not safely dispatchable
}
```

**SHACL shape** (`shapes/application_contract.shacl.ttl`):

- `dec:ApplicationContractShape sh:targetClass dec:ApplicationContract`.
- `sh:minCount 1` on all six required Convention fields (the cross_cutting list defaults to empty but each entry must be a valid Convention).
- `sh:minCount 1` on `archetype` back-reference.
- Each Convention's `name`, `body_path`, `checkable` required; `audit_id` optional.
- **Convention with `checkable: false` may exist** but its dependent TaskTypes inherit the `not-safely-dispatchable` flag (enforced downstream by FT-153 / FT-150).

**IRI vocabulary** (`vocab/application_contract.rs`): one constant per field, plus `CONVENTION_CLASS`, `CONVENTION_NAME`, `CONVENTION_BODY_PATH`, `CONVENTION_AUDIT_ID`, `CONVENTION_CHECKABLE`.

**Parser + emitter:** symmetric, FT-141 audit coverage. Convention is a sub-resource of ApplicationContract, parsed/emitted inline.

**Round-trip tests:**

- Positive: build an ApplicationContract with all six conventions + three cross-cutting entries; round-trip; assert equality.
- Negative (missing required Convention): SHACL rejects.
- Negative (Convention with empty body_path): SHACL rejects.
- Negative (`checkable: false` flags dispatchability): assert downstream `not-safely-dispatchable` propagation (this test lands as a placeholder; the propagation itself ships in FT-153 / FT-150).

**`scripts/checks/tech-detail-binding-level.sh` (v1)**:

- ADR-083's mechanical check, first version. Reads every `forge/archetypes/{id}/application/contract.md` + `infrastructure/instances/{id}/infrastructure.contract.md` pair.
- Asserts: (1) every detail in the application contract is referenced by at least one application cell prompt (the v1 check is conservative — grep the prompts dir for the convention name); (2) no detail in the application contract differs across instances; (3) no detail in an instance contract appears in an application cell prompt as a concrete value.
- Exit 0 / 1 / 2 per [ADR-013](ADR-013) two-tier convention.
- Linked as a cross-cutting TC against ADR-083 — runs through `product verify --platform`.

### State

- **New on-disk:** `application_contract.rs`, sub-module `application_contract/parser.rs`, `application_contract/emitter.rs`, `application_contract/tests.rs`, `shapes/application_contract.shacl.ttl`, `vocab/application_contract.rs`, `scripts/checks/tech-detail-binding-level.sh`.
- **Modified on-disk:** `core/ontology/mod.rs` re-exports; SHACL shape registration in `core/graph/writer.rs`.
- **Convention bodies** live outside the orchestration store under `forge/archetypes/{archetype-id}/application/conventions/{name}.md`. The store holds the path; the file holds the convention body.

### Behaviour

1. **Cluster dispatch via `add-artifact-type`**. Six cells, six emitted files. Audit teeth from FT-141.
2. **SHACL chokepoint registration**. ApplicationContractShape registered; E105 (`E105_ApplicationContractMissingRequiredConvention`) fires on writes lacking any of the six required Convention fields.
3. **Convention dispatchability propagation**. `Convention.checkable: false` propagates to every TaskType linking through `conforms_to: <convention_id>`. The propagation itself is FT-150's responsibility; this slice ships the field that drives it.
4. **`tech-detail-binding-level.sh` runs through `product verify --platform`**. Empty repo (no archetype) → trivial pass; live archetype with binding violations → fail with diagnostic.

### Invariants

- **All six required conventions present at write time.** SHACL E105 makes this structural.
- **Every convention's `body_path` resolves to an existing file.** Validated at SHACL time via a side-table file-existence check (the chokepoint reads the path, asserts existence).
- **`tech-detail-binding-level` never has a warn-band.** Binary: passes or fails. False positives in v1 (a convention legitimately not referenced by any cell prompt) are documented as known limitations; the check can be skipped per-detail via a `# tech-detail-binding-level:skip <reason>` comment in the convention body.

### Error handling

- **Missing required Convention** → E105 with the missing convention name.
- **Convention with empty `body_path`** → E106 (`E106_ConventionMissingBodyPath`).
- **`body_path` does not resolve** → E107 (`E107_ConventionBodyMissing`) — the file the path points to doesn't exist.
- **Cluster audit failure during `add-artifact-type` dispatch** → standard FT-139 rollback semantics; no Archetype writes attempted.

### Boundaries

- **In scope.** ApplicationContract + Convention struct + SHACL + IRI + parser + emitter + round-trip tests. The v1 `tech-detail-binding-level.sh` check + its cross-cutting TC linked to ADR-083. E105 / E106 / E107 SHACL errors. Conventions are referenced by ID; the body files live under `forge/archetypes/{id}/application/conventions/`.
- **Out of scope.** Convention authoring for the first archetype — lands in FT-160 (decision-cli's own contract). InfrastructureContract (FT-149). TaskType `conforms_to` field + dispatchability propagation logic — FT-150 ships the field, FT-153 ships the propagation enforcement. SeamAudits (FT-152). The audit pipeline (FT-153). The contract-authoring worker (could be a future TaskType candidate — not minted here). Convention versioning — every contract version is a new artifact; in-place amendment of accepted contracts uses the same flow as ADR amendments.

## Out of scope

- The decision-cli archetype's actual conventions (FT-160).
- Infrastructure contract substrate (FT-149).
- TaskType promotion + `conforms_to` field (FT-150) — this slice ships the back-reference shape only.
- Convention versioning + amendment workflow — uses the existing ADR-amendment shape when needed.
- LLM-driven convention authoring — manual for v1.
- A second concrete archetype's conventions.
