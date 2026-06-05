---
id: FT-147
title: 'decision-cli: Archetype artifact type and SHACL shape — the catalog layer above TaskType'
phase: 5
status: planned
depends-on: []
adrs:
- ADR-082
- ADR-084
- ADR-085
tests: []
domains:
- api
- data-model
domains-acknowledged: {}
---

## Description

The first substrate feature for [ADR-082](ADR-082). Introduces `dec:Archetype` as a first-class graph-resident artifact type — the catalog layer above the TaskType + Cell substrate that [FT-139](FT-139) shipped under [ADR-080](ADR-080).

An archetype is the unit of cross-customer reuse: a recurring *kind of system* (Self-Service Portal, Internal Admin Tool, Approval Workflow). It owns two parallel contracts (application + infrastructure), a TaskType set split by family, and audits at three scopes. This slice ships the artifact type itself — the struct + SHACL + parser + emitter + IRI vocabulary + round-trip tests. The contracts and the audit sets land in sibling slices (FT-148, FT-149, FT-152); this slice gives them a home to link to.

Implementation rides the `add-artifact-type` TaskType from [FT-141](FT-141): six cells, six emitted files, deterministic audit. The first non-cluster-prototype run of the cluster pattern — if the cluster cannot ship one of its own foundational artifact types, the pattern's coverage claim is unproven.

## Functional Specification

### Inputs

- The artifact-type registry the existing ontology surfaces — under `crates/decision-cli/src/core/ontology/`, witnessed by `feedback.rs`, `capability.rs`, `verification_bench.rs`, `worker_image.rs` ([FT-026](FT-026), [FT-035](FT-035), [FT-054](FT-054), [FT-086](FT-086)).
- The SHACL chokepoint at GraphWriter from [ADR-041](ADR-041) — extended with the new shape.
- The IRI vocabulary module at `crates/decision-cli/src/core/vocab/` — extended with archetype-class IRIs.
- The dual-provenance shapes from [FT-072](FT-072) / [FT-073](FT-073) — every Archetype carries mechanical + motivational provenance like every other artifact.
- The `add-artifact-type` TaskType from [FT-141](FT-141) — the cluster this slice's implementation rides.

### Outputs

**Rust struct** (`crates/decision-cli/src/core/ontology/archetype.rs`):

```rust
pub struct Archetype {
    pub id: NamedNode,                          // dec:archetype:<archetype-id>
    pub title: String,
    pub status: ArchetypeStatus,                // Candidate | Standard | Quarantined
    pub application_contract: NamedNode,        // → ApplicationContract IRI
    pub infrastructure_contract_template: NamedNode, // → InfrastructureContractTemplate IRI
    pub infrastructure_contract_instances: Vec<NamedNode>, // → InfrastructureContractInstance IRIs
    pub application_task_types: Vec<NamedNode>, // → TaskType IRIs with family=application
    pub infrastructure_task_types: Vec<NamedNode>, // → TaskType IRIs with family=infrastructure
    pub archetype_audits: Vec<NamedNode>,       // → ArchetypeAudit IRIs
    pub seam_audits: Vec<NamedNode>,            // → SeamAudit IRIs (must be non-empty per ADR-084)
    pub evidence: ArchetypeEvidence,            // coverage estimate, variance, instance count, contract invariance
    pub provenance: Provenance,                 // mechanical + motivational, per FT-072/073
}

pub enum ArchetypeStatus { Candidate, Standard, Quarantined }

pub struct ArchetypeEvidence {
    pub archetype_layer_estimate: f32,
    pub instance_variance: Variance,             // Low | Medium | High
    pub application_contract_held_invariant: bool,
    pub coverage_note: String,
}
```

**SHACL shape** (`crates/decision-cli/src/core/ontology/shapes/archetype.shacl.ttl`):

- `dec:ArchetypeShape sh:targetClass dec:Archetype` with one `sh:property` per field.
- `sh:minCount 1` on `application_contract`, `infrastructure_contract_template`, `status`.
- **`sh:minCount 1` on `seam_audits` → E102** (`E102_ArchetypeMissingSeamAudits`) — the gate from [ADR-084](ADR-084) §1.
- `sh:in (candidate standard quarantined)` on `status`.
- Datatype constraints on the EVIDENCE sub-shape.
- Dual-provenance imports from [FT-072](FT-072) (PROV-O `wasGeneratedBy` etc.) and the motivational-predicate fragment.

**IRI vocabulary** (`crates/decision-cli/src/core/vocab/archetype.rs`):

- `ARCHETYPE_CLASS`, `ARCHETYPE_TITLE`, `ARCHETYPE_STATUS`, one per struct field.

**Parser + emitter** (`crates/decision-cli/src/core/ontology/archetype/parser.rs`, `emitter.rs`):

- Quad-iterator → struct (parser); struct → `Vec<Quad>` (emitter). Symmetric coverage of every field per the coherence audit from FT-141.

**Round-trip tests** (`crates/decision-cli/src/core/ontology/archetype/tests.rs`):

- Positive round-trip: build an Archetype with three seam audits, emit, parse, assert structural equality.
- Negative SHACL (`seam_audits` empty): build an Archetype with zero seam audits, run SHACL validator, assert it rejects with E102.
- Negative SHACL (invalid status string): assert rejection.
- Negative SHACL (missing application_contract link): assert rejection.

**GraphWriter integration:**

- `GraphWriter::write_archetype(...)` lands as a typed method routing through the existing SHACL-enforced write path.
- Mutation of `Archetype.status: standard` outside the `dec archetype promote` CLI path is refused with E020 — the gate from [ADR-085](ADR-085) §6.

**W104 in `product graph check`:**

- Walk archetypes with `status: candidate`; if all four evidence requirements from [ADR-085](ADR-085) §1 hold, emit W104 (`W104_ArchetypePromotionReady`). Informational only.

### State

- **New on-disk:** `archetype.rs`, `shapes/archetype.shacl.ttl`, `vocab/archetype.rs`, `archetype/parser.rs`, `archetype/emitter.rs`, `archetype/tests.rs`.
- **Modified on-disk:** `crates/decision-cli/src/core/ontology/mod.rs` (re-export the new module); `crates/decision-cli/src/core/graph/writer.rs` (add the typed write method).
- **Orchestration-store schema change:** new SHACL shape registered with the chokepoint; new IRI vocabulary entries. Backwards-compatible — existing archetype-free stores keep working.

### Behaviour

1. **Cluster dispatch via `add-artifact-type`**. This slice's implementation runs through the FT-141 cluster: `rust_struct` → `shacl_shape` + `iri_module_consts` → `parser` + `emitter` → `round_trip_tests`. The cluster's coherence audit runs against the six emitted files; SHACL field coverage, IRI const reachability, parser+emitter field coverage, both round-trip cases, no Python files.
2. **GraphWriter SHACL extension**. Register the new shape with the chokepoint at startup; E102 fires on Archetype writes that violate `seam_audits sh:minCount 1`.
3. **Status promotion gate**. Mutation paths checking `Archetype.status` writes route only through `dec archetype promote` / `dec archetype demote` / quarantine paths; any other path is refused with E020.
4. **W104 in graph check**. After every graph check, iterate archetypes; for each `status: candidate` archetype, evaluate the four-piece evidence check from ADR-085 §1; emit W104 with the archetype id if all four hold.

### Invariants

- **Every Archetype carries dual provenance.** Mechanical (PROV-O `wasGeneratedBy` etc.) and motivational (predicate vocabulary) are required at write time. No archetype escapes the FT-073 chokepoint.
- **`seam_audits` is non-empty at register time.** E102 makes this structural.
- **`status: standard` mutations are CLI-gated.** No code path other than `dec archetype promote` can flip the status to `standard`.
- **Application contract invariance is recorded.** `application_contract_held_invariant: bool` is required; defaults to `false` and must be explicitly set to `true` by the EVIDENCE author once contract regression evidence accumulates.

### Error handling

- **Empty `seam_audits` at write** → E102 (`E102_ArchetypeMissingSeamAudits`); refuses the write; surfaces via the GraphWriter error type.
- **Invalid `status` enum value** → E103 (`E103_ArchetypeInvalidStatus`); SHACL `sh:in` rejection.
- **Missing application_contract or infrastructure_contract_template link** → E104 (`E104_ArchetypeMissingContractLink`); SHACL `sh:minCount 1` rejection.
- **Mutation of `Archetype.status: standard` outside CLI** → E020 (existing code) with the path identifier in the diagnostic.
- **SHACL validator unrunnable (shape file missing)** → SHACL-Unrunnable; surfaces as a startup error, not a per-write error.

### Boundaries

- **In scope.** The Archetype artifact type (Rust struct, SHACL shape, IRI vocab, parser, emitter, round-trip tests). E102 / E103 / E104 SHACL errors. The W104 graph-check warning. The status-promotion E020 gate. `GraphWriter::write_archetype` typed surface. Four exit-criteria TCs: round-trip equality, E102 enforcement, W104 emission, E020 status-mutation gate.
- **Out of scope.** ApplicationContract artifact type (FT-148). InfrastructureContract template / instance (FT-149). TaskType promotion + family / conforms_to fields (FT-150). SeamAudit + ArchetypeAudit artifact types (FT-152). The audit pipeline (FT-153). The escape hatch (FT-154). The classifier worker (FT-155). The extraction worker (FT-156). The dispatch planner (FT-157). `dec archetype` CLI (FT-158). The first archetype (FT-160). The CLI commands `dec archetype promote / demote` — declared by the SHACL gate's existence here but implemented in FT-158.

## Out of scope

- ApplicationContract / InfrastructureContract artifact types — sibling slices FT-148 / FT-149.
- TaskType promotion to first-class artifact + family / conforms_to fields — FT-150.
- SeamAudit / ArchetypeAudit artifact types — FT-152.
- Three-scope audit pipeline + classifier + dispatcher + planner — FT-153/155/157.
- CLI verbs `dec archetype list / show / promote / demote` — FT-158.
- The first archetype's actual contracts and audits — FT-160.
- Migration of existing TaskType feature_specs (FT-139..FT-144) into the new Archetype linkage — lands as part of FT-150 + FT-160.
