---
id: FT-152
title: 'decision-cli: SeamAudit + ArchetypeAudit artifact types with monolith-bar evidence fields'
phase: 5
status: planned
depends-on:
- FT-147
- FT-149
- FT-167
adrs:
- ADR-082
- ADR-084
tests: []
domains:
- api
- data-model
- observability
domains-acknowledged: {}
---

## Description

Two related artifact types for [ADR-082](ADR-082) §4 and [ADR-084](ADR-084): `dec:SeamAudit` (the load-bearing audit class — application output ⟷ infrastructure output) and `dec:ArchetypeAudit` (conformance-to-contract audits, distinct from seam audits because scope differs).

SeamAudit is the audit DDD's analysis identifies as the most damaging-failure firewall: it catches the misconfigured managed identity, the connection-string mismatch, the role IaC granted but the app doesn't assume. ADR-084 makes it mandatory and load-bearing. This slice ships the typed artifact + the monolith-bar evidence fields + the W104 readiness check infrastructure + SHACL gating that lets [FT-147](FT-147)'s E102 (Archetype missing seam audits) fire correctly.

ArchetypeAudit is a sibling type — same shape but different scope. It checks that *any dispatched cluster's output* conforms to a single contract (e.g., slice-conforms-to-clean-architecture, endpoint-contract-test-alignment). They are split because failure responsibility is different: a SeamAudit failure means the application and infrastructure TaskTypes produced mutually incoherent output; an ArchetypeAudit failure means one cluster violated the contract its own family is bound to.

## Functional Specification

### Inputs

- `Archetype` from [FT-147](FT-147) — links to `seam_audits` and `archetype_audits`.
- `ApplicationContract` + `Convention` from [FT-148](FT-148) — archetype audits reference conventions.
- `InfrastructureContractInstance.iac_outputs` from [FT-149](FT-149) — seam audits consume the output set.
- The `add-artifact-type` TaskType ([FT-141](FT-141)) — implementation cluster (runs twice, once per artifact type).
- The monolith-bar requirement from ADR-084 §2 and the regression-evidence requirement from ADR-084 §5.

### Outputs

**Rust structs** (`crates/decision-cli/src/core/ontology/audit.rs`):

```rust
pub struct SeamAudit {
    pub id: NamedNode,
    pub archetype: NamedNode,
    pub family: SeamAuditFamily,                  // AppConfigMatchesIacOutputs | AppIdentityMatchesIacRoles | AppResourceExpectationsMet | Custom(name)
    pub name: String,
    pub description: String,
    pub runner: String,                            // matches the test-criterion runner shape (bash | python | ...)
    pub runner_args: String,
    pub runner_timeout: Duration,
    pub monolith_bar: MonolithBar,                 // Passes | CandidateAuditWeak | Unrunnable
    pub monolith_bar_evidence: String,             // free-text + optional → RegressionEvidence
    pub regression_evidence: Vec<NamedNode>,       // → RegressionEvidence IRIs
    pub provenance: Provenance,
}

pub enum SeamAuditFamily {
    AppConfigMatchesIacOutputs,
    AppIdentityMatchesIacRoles,
    AppResourceExpectationsMet,
    Custom(String),
}

pub enum MonolithBar { Passes, CandidateAuditWeak, Unrunnable }

pub struct ArchetypeAudit {
    pub id: NamedNode,
    pub archetype: NamedNode,
    pub name: String,                              // e.g. "slice-conforms-to-clean-architecture"
    pub validates_convention: NamedNode,           // → Convention IRI from the ApplicationContract
    pub runner: String,
    pub runner_args: String,
    pub runner_timeout: Duration,
    pub monolith_bar: MonolithBar,
    pub monolith_bar_evidence: String,
    pub provenance: Provenance,
}

pub struct RegressionEvidence {
    pub id: NamedNode,
    pub audit: NamedNode,                          // → SeamAudit or ArchetypeAudit
    pub instance: NamedNode,                       // → InfrastructureContractInstance (the known-good instance used)
    pub drift_caught: String,                      // description of the drift the audit caught
    pub regenerated_at: DateTime,
    pub provenance: Provenance,
}
```

**SHACL shapes** (`shapes/audit.shacl.ttl`):

- `dec:SeamAuditShape sh:targetClass dec:SeamAudit` with required `archetype`, `family`, `name`, `runner`, `runner_args`, `monolith_bar`.
- `dec:ArchetypeAuditShape sh:targetClass dec:ArchetypeAudit` with required `archetype`, `name`, `validates_convention`, `runner`, `runner_args`, `monolith_bar`.
- `dec:RegressionEvidenceShape sh:targetClass dec:RegressionEvidence` with required `audit`, `instance`, `drift_caught`, `regenerated_at`.
- Cross-shape constraint: **`monolith_bar: Passes` requires `regression_evidence sh:minCount 1`** — E119 (`E119_MonolithBarPassesWithoutEvidence`). This is the ADR-084 §5 evidence requirement made structural.
- Cross-shape constraint: every required SeamAudit family from ADR-084 §3 — `AppConfigMatchesIacOutputs`, `AppIdentityMatchesIacRoles`, `AppResourceExpectationsMet` — must have at least one SeamAudit per Archetype. E120 (`E120_ArchetypeMissingRequiredSeamAuditFamily`).

**W104 implementation in `product graph check`:**

- Walk archetypes with `status: candidate`.
- For each, check ADR-085's four evidence requirements; the third (every SeamAudit at `monolith_bar: Passes`) reads from this slice's typed surface.
- Emit W104 (`W104_ArchetypePromotionReady`) when all four hold.

**Runner registry extension:**

- Audits use the same runner contract as test criteria ([ADR-013](ADR-013)): `bash | python | cargo-test | pytest | ...`, exit 0 = pass, 1 = fail, 2 = unrunnable.
- A new helper `audit::run_audit(audit_id) -> AuditOutcome` invokes the runner with the declared args + timeout; returns `AuditOutcome::Passed | Failed { stderr } | Unrunnable { stderr }`.
- The helper is called by FT-153's three-scope pipeline.

**Test coverage:**

- Positive: SeamAudit round-trip + ArchetypeAudit round-trip + RegressionEvidence round-trip.
- Negative (`monolith_bar: Passes` without regression evidence) → E119.
- Negative (Archetype missing all three required seam-audit families) → E120 (this overlaps with E102 from FT-147 but is more granular).
- Positive (runner invocation): a fixture bash audit returning exit 0; `audit::run_audit` returns `Passed`.
- Negative (runner invocation): a fixture bash audit returning exit 1; returns `Failed { stderr }`.
- W104 emission: a candidate archetype with three SeamAudits at `Passes` + 3 instances + contract invariance proven; W104 fires.

### State

- **New on-disk:** `audit.rs`, sub-module `audit/{parser,emitter,tests,runner}.rs`, `shapes/audit.shacl.ttl`, `vocab/audit.rs`.
- **Modified on-disk:** ontology re-exports; SHACL registration; W104 added to graph-check output.
- **Audit runner scripts** live under `forge/archetypes/{archetype-id}/audits/{seam,archetype}/<name>.{sh,py}` — same path convention as task-type cells under FT-141.

### Behaviour

1. **Cluster dispatch via `add-artifact-type` × 3**. SeamAudit + ArchetypeAudit + RegressionEvidence — three artifact types in one slice. The cluster runs three times. Coherence audit teeth from FT-141 apply to each.
2. **SHACL chokepoint**: E119 + E120 fire at audit / archetype writes.
3. **`audit::run_audit` helper**. Reusable function for FT-153's pipeline. Honours runner contract.
4. **W104 emitted by graph check**. The check reads ADR-085's four evidence requirements; the third reads from this slice's typed surface.

### Invariants

- **`monolith_bar: Passes` requires at least one RegressionEvidence linked.** E119.
- **Every archetype has at least one SeamAudit per required family.** E120 (more granular than FT-147's E102).
- **RegressionEvidence is immutable.** Once linked from a SeamAudit, mutations are refused via SHACL chokepoint (the evidence record is graph-resident audit trail).
- **Runner contract is honoured.** Audits return exit 0 / 1 / 2; `audit::run_audit` maps cleanly to AuditOutcome variants.

### Error handling

- **E119** — `monolith_bar: Passes` without regression_evidence.
- **E120** — Archetype missing required seam-audit family.
- **Runner exit non-{0,1,2}** → `AuditOutcome::Unrunnable` with the actual exit code in stderr.
- **Runner timeout** → `AuditOutcome::Unrunnable` with the timeout indicator.

### Boundaries

- **In scope.** SeamAudit + ArchetypeAudit + RegressionEvidence artifact types; SHACL shapes + E119 / E120; `audit::run_audit` helper; W104 implementation; eight test cases (round-trips + negatives + runner + W104).
- **Out of scope.** Authoring the first archetype's audits — FT-160. The three-scope dispatch pipeline — FT-153. The classifier / dispatcher / planner — FT-155 / FT-157. Custom seam-audit family beyond the three required — extensible by `SeamAuditFamily::Custom(name)` but no first user. Audit authoring workers — possible TaskType candidate; not minted here. Cross-archetype shared audits — out of v1.

## Out of scope

- First archetype's audits — FT-160.
- Three-scope dispatch pipeline — FT-153.
- Audit authoring workers (TaskType candidate).
- Cross-archetype shared audits.
- Advanced runner contracts (parallel runners, sandboxed runners).
