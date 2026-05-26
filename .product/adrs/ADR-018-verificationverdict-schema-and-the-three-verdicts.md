---
id: ADR-018
title: VerificationVerdict schema and the three verdicts
status: accepted
features:
- FT-020
- FT-023
supersedes: []
superseded-by: []
domains:
- data-model
scope: domain
content-hash: sha256:8a23203c0e42dff4466dbe777617e1b253eb9f52435deb403580995baa809665
source-files:
- crates/decision-cli/src/core/ontology/verdict.rs
- crates/decision-cli/src/core/ontology/verdict.ttl
---

## Context

[ADR-017](ADR-017) requires every action session to be paired with an interpretation session that produces a verdict. The verdict is the artifact that gates dispatch completion. Its schema is therefore load-bearing: a verdict that can be parsed multiple ways defeats the purpose of structural pairing.

The verdict must answer three questions:

1. **Did the produced artifact satisfy the originating feature_spec and its TCs?** Yes / no / partially.
2. **What is the evidence?** A rationale, referencing specific TCs, ADRs, or feature_spec lines.
3. **What happens next?** Approve, reject outright, or amend.

The vocabulary needs to be small enough that verifier workers can produce reliable structured output, and rich enough that downstream roles (slice 3 feedback, Phase C fitness functions) can distinguish meaningful failure modes.

## Decision

**`dec:VerificationVerdict` is a first-class graph artifact with three verdicts and a fixed SHACL shape.**

### Verdicts (`dec:verdict`)

| Verdict | Meaning | Dispatch transition |
|---|---|---|
| `approved` | The produced artifact satisfies the feature_spec and its TCs. | `DispatchGroup` → `complete`. |
| `rejected` | The produced artifact does not satisfy the feature_spec, and the verifier cannot specify a corrective action. The action was wrong in kind, not in detail. | `DispatchGroup` → `interpretation-rejected`. |
| `amendment-required` | The produced artifact is on the right track but needs specific changes the verifier can describe. | `DispatchGroup` → `awaiting-amendment`. A follow-up dispatch consumes the verdict's `dec:amendmentGuidance` as additional context. |

### SHACL shape

```turtle
@prefix dec:  <https://decision-cli.dev/ns#> .
@prefix sh:   <http://www.w3.org/ns/shacl#> .
@prefix prov: <http://www.w3.org/ns/prov#> .
@prefix xsd:  <http://www.w3.org/2001/XMLSchema#> .

dec:VerificationVerdictShape a sh:NodeShape ;
    sh:targetClass dec:VerificationVerdict ;
    sh:property [
        sh:path dec:verdict ;
        sh:in ( "approved" "rejected" "amendment-required" ) ;
        sh:minCount 1 ; sh:maxCount 1 ;
    ] ;
    sh:property [
        sh:path dec:rationale ;
        sh:datatype xsd:string ;
        sh:minCount 1 ; sh:maxCount 1 ;
        sh:minLength 20 ;       # block trivial "ok" responses
    ] ;
    sh:property [
        sh:path prov:wasGeneratedBy ;        # interpretation session
        sh:minCount 1 ; sh:maxCount 1 ;
    ] ;
    sh:property [
        sh:path prov:used ;                  # action artifacts consumed
        sh:minCount 1 ;
    ] ;
    sh:property [
        sh:path dec:inStream ;
        sh:minCount 1 ; sh:maxCount 1 ;      # ADR-005
    ] ;
    # Conditional: rejected/amendment-required MUST cite ≥ 1 TC or ADR.
    sh:sparql [
        sh:message "rejected and amendment-required verdicts must cite at least one violated TC or ADR via dec:violates" ;
        sh:select """
            PREFIX dec: <https://decision-cli.dev/ns#>
            SELECT $this WHERE {
              $this dec:verdict ?v .
              FILTER(?v IN ("rejected", "amendment-required"))
              FILTER NOT EXISTS { $this dec:violates ?ref }
            }
        """ ;
    ] ;
    sh:property [
        sh:path dec:amendmentGuidance ;
        # required iff verdict = amendment-required (enforced by sh:sparql below)
        sh:datatype xsd:string ;
        sh:maxCount 1 ;
    ] ;
    sh:sparql [
        sh:message "amendment-required verdicts must carry dec:amendmentGuidance" ;
        sh:select """
            PREFIX dec: <https://decision-cli.dev/ns#>
            SELECT $this WHERE {
              $this dec:verdict "amendment-required" .
              FILTER NOT EXISTS { $this dec:amendmentGuidance ?g }
            }
        """ ;
    ] .
```

### Required fields summary

| Field | Cardinality | Notes |
|---|---|---|
| `dec:verdict` | 1 | One of the three above. |
| `dec:rationale` | 1 | Free-form text, ≥ 20 chars. |
| `prov:wasGeneratedBy` | 1 | The `InterpretationSession`. |
| `prov:used` | ≥ 1 | The action artifacts the verdict refers to (typically the produced `CodeChange` + the originating `FeatureSpec`). |
| `dec:inStream` | 1 | ValueStream link per [ADR-005](ADR-005). |
| `dec:violates` | ≥ 1 if `rejected`/`amendment-required` | URI references to violated TCs or ADRs. |
| `dec:amendmentGuidance` | 1 if `amendment-required`, else 0 | Free-form text the next action consumes. |

### Why three verdicts, not two

A binary approve/reject vocabulary collapses two distinct corrective paths into one. "The implementer wrote the wrong feature" is categorically different from "the implementer wrote roughly the right thing but missed an edge case." Conflating them forces every rejection to be a full restart, which makes verifier behavior conservative (over-approving) and burns LLM budget on re-running entire implementations for small corrections.

`amendment-required` captures the case where the verifier has *specific actionable guidance*. It is the entry point for the Phase B re-dispatch flow.

### Why ≥ 20 char rationale

A trivial "ok" or "looks good" rationale defeats the audit trail. The 20-character minimum is a floor, not a quality bar; it filters obvious noise without trying to score prose quality.

## Rejected alternatives

- **Binary verdict (approve/reject only).** Rejected — see above.
- **Free-form verdict string with no `sh:in`.** Rejected: invites verifier prose drift; downstream queries can't aggregate.
- **No required rationale.** Rejected: makes verdicts unauditable.
- **Numeric confidence score (e.g. 0.0–1.0).** Rejected for Phase A: introduces a metric whose calibration is itself an open question. Revisit once we have enough action-interpretation pairs to define what "0.8 confident" actually means.
- **Verifier directly mutates feature_spec status.** Rejected: confuses the verifier's authority. The verdict is evidence; the dispatch state machine consumes the evidence. The verifier does not own feature lifecycle.

## Consequences

**Positive:**
- Downstream SPARQL is straightforward: "approved verdicts grouped by feature" is one query.
- The schema is small enough that Pydantic models on the worker side are 5 lines.
- Phase C fitness functions can compute action-interpretation agreement, rejection rates per role, and time-to-amendment with no schema changes.

**Negative / accepted costs:**
- The three-state vocabulary needs to hold for the lifetime of Phase A and B. Adding a fourth verdict later is a schema migration.

**Enforcement:**
- SHACL shape lives in `crates/decision-cli/src/core/ontology/` (under [FT-006](FT-006)'s ontology) — extended in slice 2 (FT-020).
- Write-side validation at the `StreamWriter` ([ADR-005](ADR-005)) chokepoint refuses malformed verdicts before they reach the store.
- A cross-cutting TC asserts every approved `DispatchGroup` has a verdict whose `dec:verdict` is `approved` and whose `dec:rationale` is non-empty.

## Status

Proposed. Linked to FT-020 (the schema-shaped feature that lands the SHACL extension).
