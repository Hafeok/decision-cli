---
id: FT-027
title: 'decision-cli: Feedback lifecycle state machine'
phase: 2
status: complete
depends-on:
- FT-026
adrs:
- ADR-024
tests:
- TC-035
- TC-038
- TC-092
domains: []
domains-acknowledged:
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-027's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-027 neither emits nor routes feedback.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-027 has no feedback to gate.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-027 produces no feedback artifacts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-027 produces no action/interpretation pair.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-027 does not introduce or modify a role catalog entry.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-027 produces no new Session or event type and inherits lineage from the harness.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-027's code is reorganised under that migration, not by this feature.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-027 does not author or modify a fitness-function artifact.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-027 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-027 neither emits nor consumes verdicts.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-027 does not cross or alter that boundary.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-027 is out of scope for the pairing.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-027 runs after the working directory is resolved and does not re-discover it.
---

## Description

Enforce the feedback lifecycle state machine from [ADR-024](ADR-024): seven states, eight valid transitions, only-forward, validated at write time. Sits on top of [FT-026](FT-026)'s feedback schema; consumed by [FT-029](FT-029) (routing), [FT-031](FT-031) (workers), [FT-032](FT-032) (blocking semantics), and [FT-033](FT-033) (CLI close).

## Functional Specification

### Inputs

- The `dec:Feedback` schema and `lifecycleState` field from [FT-026](FT-026).
- The `StreamWriter` chokepoint ([ADR-005](ADR-005)).

### Outputs

- SHACL extensions on `dec:FeedbackShape`:
  - `sh:in` on `dec:lifecycleState` constraining to the seven states.
  - `sh:sparql` constraints enforcing state-specific required fields (e.g. `addressed` requires `dec:addressingArtifact`).
- Rust state machine `core::feedback::lifecycle`:
  - `enum LifecycleState { Produced, Routed, Received, Addressed, Closed, Rejected, Superseded }`
  - `fn next_states(from: LifecycleState) -> &'static [LifecycleState]` — table of valid transitions.
  - `fn validate_transition(from: LifecycleState, to: LifecycleState) -> Result<(), TransitionError>`.
- `StreamWriter` transition validation: when committing a mutation that updates `dec:lifecycleState` on an existing `Feedback`, read the prior state and call `validate_transition`. Refuse invalid transitions with `WriterError::InvalidLifecycleTransition { from, to }`.
- A small helper `core::feedback::transition::apply(store, feedback_iri, new_state, evidence) -> Result<...>` that handles the read-validate-write cycle for transition mutations.

### State

- Per-feedback mutations: each lifecycle transition is a new mutation through `StreamWriter`. Prior states remain in named-graph history per the slice-1 graph-as-state design ([ADR-002](ADR-002)).

### Behaviour

1. Extend SHACL with the `sh:in` and `sh:sparql` constraints from [ADR-024](ADR-024).
2. Author `core::feedback::lifecycle` with the enum, the next-states table, and the validator.
3. Add the `transition::apply` helper that callers (FT-029 routing handler, FT-031 worker harness, FT-033 CLI close) use.
4. Extend `StreamWriter` to call the validator on lifecycle-update mutations.
5. Per slice-level SDP: this module is `core::feedback::lifecycle`. Every caller imports from here.

### Invariants

- No `Feedback` artifact ever transitions to a state outside its `next_states` set.
- Terminal states (`closed`, `rejected`, `superseded`) have empty `next_states` — no further transitions possible.
- Required-field invariants from [ADR-024](ADR-024) hold for every persisted state.
- Rust-side and SHACL-side validation agree: any transition the Rust API allows is also SHACL-valid, and vice versa.

### Error handling

- Invalid transition attempt → `TransitionError::InvalidTransition { from, to }` from Rust validator; `WriterError::InvalidLifecycleTransition` from `StreamWriter` (with the same shape, surfaced through the writer error type).
- Required field missing for target state → `TransitionError::MissingField { state, field }`.
- Attempt to transition from a terminal state → `TransitionError::TerminalState { state }`.

### Boundaries

- **In scope.** State enum, next-states table, validator, write-side enforcement, SHACL constraints.
- **Out of scope.** Triggering transitions (the routing subscription in [FT-029](FT-029) triggers `produced → routed`; workers trigger `produced` via SDK; CLI triggers `addressed → closed` in [FT-033](FT-033)).

## Out of scope

- Reverse transitions (rejected per ADR-024).
- Lifecycle hooks (e.g. "on transition to closed, notify role X") — Phase B at earliest, via subscriptions.
- Concurrent-transition deduplication — `StreamWriter`'s single-writer invariant covers slice-3 cases.
