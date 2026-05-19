---
id: FT-027
title: 'decision-cli: Feedback lifecycle state machine'
phase: 2
status: planned
depends-on:
- FT-026
adrs:
- ADR-002
- ADR-024
tests:
- TC-035
- TC-038
domains: []
domains-acknowledged: {}
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
