---
id: FT-055
title: 'decision-cli: RoleBinding, EscalationStep, and EscalationTrigger artifact types'
phase: 2
status: planned
depends-on:
- FT-054
adrs:
- ADR-033
- ADR-034
tests:
- TC-101
domains:
- data-model
domains-acknowledged: {}
---

## Description

Introduce three coupled artifact types that together encode escalation policy:

- `dec:RoleBinding` — declares the default capability for a role plus an ordered list of escalation steps.
- `dec:EscalationStep` — an ordered list entry: a candidate capability plus the set of triggers that selects it.
- `dec:EscalationTrigger` — a single trigger drawn from the controlled vocabulary in [ADR-034](ADR-034).

This feature lands the ontology terms, SHACL shapes, and Rust types. Catalog content (concrete bindings for the PRD-defined roles) lives in [FT-058](FT-058); dispatcher consumption lives in [FT-061](FT-061) (default capability only) and [FT-062](FT-062) (escalation loop).

## Functional Specification

### Inputs

- The embedded base ontology ([FT-006](FT-006)) gains three classes and their predicates.
- The capability class from [FT-054](FT-054) is in place so `dec:default_capability` and `dec:step_capability` can reference it.
- The trigger vocabulary fixed by [ADR-034](ADR-034) is encoded as a SHACL `sh:in` constraint on `dec:trigger_signal`.

### Outputs

- New ontology terms (under `dec:`):
  - Class `dec:RoleBinding`.
    - `dec:role_id` (xsd:string, required; references the role's id in the role catalog from [FT-030](FT-030); not a class IRI to avoid coupling).
    - `dec:default_capability` (object property, required; range `dec:Capability`).
    - `dec:escalation_steps` (object property, optional; range is an `rdf:List` of `dec:EscalationStep`).
    - `dec:version` (xsd:integer, required; ≥ 1).
    - `dec:active` (xsd:boolean, required; default true).
    - `dec:supersedes` (optional; `dec:RoleBinding` IRI).
    - `dec:bootstrap_source` (xsd:string, optional).
  - Class `dec:EscalationStep`.
    - `dec:step_capability` (object property, required; range `dec:Capability`).
    - `dec:triggers` (object property, required; range is an `rdf:Bag` of `dec:EscalationTrigger`; `sh:minCount 1`).
  - Class `dec:EscalationTrigger`.
    - `dec:trigger_signal` (xsd:string, required; enum: see vocabulary below).

### Trigger signal vocabulary

The `sh:in` constraint on `dec:trigger_signal` accepts these literal values, fixed by [ADR-034](ADR-034):

- Stakes: `stakes_routine`, `stakes_elevated`, `stakes_foundational`
- Confidence: `confidence_below_0.5`, `confidence_below_0.7`, `confidence_below_0.9`
- Audit: `audit_pass`, `audit_fail`
- Attempts: `prior_attempts_ge_1`, `prior_attempts_ge_2`, `prior_attempts_ge_3`, `prior_attempts_ge_4`, `prior_attempts_ge_5`
- Feedback: `feedback_contradiction`, `feedback_unimplementable_critical`, `feedback_gap`, `feedback_scope_issue`

Extending the vocabulary is itself a feature_spec: extend the SHACL enum, extend the dispatcher's switch in [FT-062](FT-062), revalidate existing artifacts.

### State

- Embedded ontology and shapes bytes grow by ~80 lines.
- `OntologyHandle::version()` bumps; init flows refresh the ontology hash.

### Behaviour

1. Extend the ontology Turtle with the three classes and their predicates.
2. Extend the shapes Turtle with:
   - `dec:RoleBindingShape`:
     - `sh:targetClass dec:RoleBinding`.
     - `sh:property` constraints for `role_id`, `default_capability`, `version`, `active`.
     - A `sh:sparql` constraint enforcing at most one active binding per `role_id`.
     - A `sh:sparql` constraint enforcing that `default_capability` references a capability whose `status ≠ eol`.
   - `dec:EscalationStepShape`:
     - `sh:targetClass dec:EscalationStep`.
     - `sh:property` constraints for `step_capability` and `triggers` (`sh:minCount 1` on triggers).
   - `dec:EscalationTriggerShape`:
     - `sh:targetClass dec:EscalationTrigger`.
     - `sh:in` constraint on `trigger_signal` with the vocabulary above.
3. Add Rust types under `core::ontology::role_binding`:
   ```rust
   pub struct RoleBinding {
       pub role_id: String,
       pub default_capability: CapabilityRef, // (id, version)
       pub escalation_steps: Vec<EscalationStep>, // RDF list order preserved
       pub version: u32,
       pub active: bool,
       pub supersedes: Option<RoleBindingRef>,
   }
   pub struct EscalationStep {
       pub step_capability: CapabilityRef,
       pub triggers: Vec<TriggerSignal>, // bag → vec; order unimportant
   }
   pub enum TriggerSignal {
       StakesRoutine, StakesElevated, StakesFoundational,
       ConfidenceBelow05, ConfidenceBelow07, ConfidenceBelow09,
       AuditPass, AuditFail,
       PriorAttemptsGe1, PriorAttemptsGe2, PriorAttemptsGe3, PriorAttemptsGe4, PriorAttemptsGe5,
       FeedbackContradiction, FeedbackUnimplementableCritical, FeedbackGap, FeedbackScopeIssue,
   }
   ```
4. Add SPARQL helpers:
   - `core::graph::role_binding::active_for_role(role_id) -> Option<RoleBinding>` — resolves through `dec:supersedes`, picks the latest active.
   - `core::graph::role_binding::list_all_active() -> Vec<RoleBinding>` — for `dec binding list`.

### Invariants

- At most one `dec:RoleBinding` per `role_id` has `dec:active = true`.
- The RDF list at `dec:escalation_steps` preserves order (this is what `rdf:List` guarantees by construction); the Rust deserialiser walks the list in order.
- `dec:triggers` is an `rdf:Bag` (order does not matter; the dispatcher OR-evaluates the triggers per [ADR-034](ADR-034)).
- `escalation_steps` may be empty for bounded-classification roles (`test_interpreter`, `feedback_class_triager` per [ADR-037](ADR-037)).
- A `RoleBinding` whose `default_capability.supports_tool_calling = false` cannot be active for a role whose worker requires tool calling — this check happens at dispatcher resolution time per [FT-061](FT-061), not at SHACL time (the catalog may carry preview entries).

### Error handling

- Missing required field → SHACL violation.
- Unknown `trigger_signal` literal → SHACL `sh:in` violation; the writer must amend.
- Two active bindings for the same `role_id` → `sh:sparql` violation; the writer must supersede the prior one explicitly.
- Empty `triggers` set on an `EscalationStep` → SHACL `sh:minCount 1` violation.

### Boundaries

- **In scope.** Ontology classes, SHACL shapes, Rust types, SPARQL query helpers.
- **Out of scope.** Catalog content — [FT-058](FT-058). Dispatcher reading bindings — [FT-061](FT-061). Escalation loop — [FT-062](FT-062). Trigger signal computation — [FT-062](FT-062). Role catalog entries — [FT-030](FT-030).

## Out of scope

- Trigger expressions beyond the closed vocabulary (rejected by [ADR-034](ADR-034)).
- Per-bundle binding overrides (rejected by [ADR-035](ADR-035) — stakes is per-bundle but bindings are per-role).
- Authority changes — [FT-030](FT-030)'s authority declarations are orthogonal to capability bindings.
