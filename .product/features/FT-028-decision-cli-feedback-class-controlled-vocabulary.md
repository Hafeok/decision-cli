---
id: FT-028
title: 'decision-cli: Feedback class controlled vocabulary'
phase: 2
status: complete
depends-on:
- FT-026
adrs:
- ADR-023
tests:
- TC-034
domains: []
domains-acknowledged:
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-028's code is reorganised under that migration, not by this feature.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-028 has no feedback to gate.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-028 produces no new Session or event type and inherits lineage from the harness.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-028 neither emits nor consumes verdicts.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-028 runs after the working directory is resolved and does not re-discover it.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-028 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-028's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-028 does not introduce or modify a role catalog entry.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-028 is out of scope for the pairing.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-028 produces no feedback artifacts.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-028 does not author or modify a fitness-function artifact.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-028 produces no action/interpretation pair.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-028 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-028 does not cross or alter that boundary.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-028 neither emits nor routes feedback.
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
