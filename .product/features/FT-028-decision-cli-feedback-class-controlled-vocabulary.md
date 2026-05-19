---
id: FT-028
title: 'decision-cli: Feedback class controlled vocabulary'
phase: 2
status: planned
depends-on:
- FT-026
adrs:
- ADR-023
tests:
- TC-034
domains: []
domains-acknowledged: {}
---

## Description

Land the controlled vocabulary for `dec:feedbackClass` from [ADR-023](ADR-023): six values, enforced by SHACL `sh:in`, mapped to a Rust enum. Together with [FT-026](FT-026) (schema) and [FT-027](FT-027) (lifecycle), this is the third leg of the feedback schema substrate.

## Functional Specification

### Inputs

- The `dec:feedbackClass` predicate from [FT-026](FT-026).
- The class definitions from [ADR-023](ADR-023).

### Outputs

- SHACL extension on `dec:FeedbackShape`:
  ```turtle
  sh:property [
      sh:path dec:feedbackClass ;
      sh:in ( "gap" "contradiction" "unimplementable"
              "scope-issue" "defect" "capability-request" ) ;
      sh:minCount 1 ; sh:maxCount 1 ;
  ] ;
  ```
- Rust enum `core::feedback::class::FeedbackClass`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(rename_all = "kebab-case")]
  pub enum FeedbackClass {
      Gap,
      Contradiction,
      Unimplementable,
      ScopeIssue,
      Defect,
      CapabilityRequest,
  }

  impl FeedbackClass {
      pub fn as_iri_value(&self) -> &'static str;
      pub fn from_iri_value(s: &str) -> Option<Self>;
      pub fn default_target_role(&self) -> &'static str;       // per ADR-026
      pub fn default_disposition(&self) -> Disposition;         // per ADR-025
      pub fn all() -> &'static [FeedbackClass];                 // for iteration/seeding
  }
  ```
- Tests that the six string values round-trip through `as_iri_value` / `from_iri_value`.

### State

- No runtime state. The enum is compile-time.

### Behaviour

1. Add the `sh:in` constraint to the SHACL shape.
2. Author the Rust enum with serde rename to kebab-case (matching the IRI string literals).
3. Provide the `default_target_role` and `default_disposition` helpers per ADR-026 and ADR-025.
4. Expose from `core::feedback::class` and re-export at `core::feedback`.

### Invariants

- The `sh:in` list and the Rust enum variants are kept in lockstep — adding a class requires updating both in the same request (ADR-023 amendment procedure).
- Every persisted `Feedback` has `dec:feedbackClass` in the six-value set.
- `from_iri_value("gap")` returns `Some(FeedbackClass::Gap)`; all six round-trip.

### Error handling

- Unknown class string at write time → SHACL violation, refused.
- Unknown class string in Rust deserialisation → serde error (caller handles).

### Boundaries

- **In scope.** SHACL `sh:in`, Rust enum, helpers, tests.
- **Out of scope.** Routing logic (uses `default_target_role` but lives in [FT-029](FT-029)). Disposition logic (lives in [FT-032](FT-032)).

## Out of scope

- Adding new classes outside the ADR-023 amendment procedure.
- Hierarchical class structure (rejected per ADR-023).
