---
id: FT-010
title: 'decision-cli: Value stream scope enforcement'
phase: 1
status: complete
depends-on:
- FT-008
- FT-009
adrs:
- ADR-008
tests:
- TC-007
- TC-014
domains: []
domains-acknowledged:
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-010 produces no feedback artifacts.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-010 is out of scope for the pairing.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-010 produces no feedback artifacts.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-010's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-010's code is reorganised under that migration, not by this feature.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-010 does not author or modify a fitness-function artifact.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-010 neither emits nor routes feedback.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-010 neither emits nor consumes verdicts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-010 produces no action/interpretation pair.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-010 has no feedback to gate.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-010 does not introduce or modify a role catalog entry.
---

## Description

Per **ADR-005 (Value stream as a graph-resident scope, enforced at command time)** every Session / Goal / Dispatch / Event written through the writer carries a `dec:inStream` triple linking to the active ValueStream, and every goal-driven invocation validates its goal verb against the stream's persisted authorized-goals before any role dispatches.

The active stream id is fixed at process start from the discovered `.dec/` (ADR-012); it cannot change at runtime.

See `decision-cli-slice-1-bounds.md` §3.4, §6.1.

## Functional Specification

### Inputs

- A reference to the persisted `ValueStream` (loaded from FT-009's store at process start).
- A goal verb from the command line (e.g., `ship`).
- Every mutation routed through the writer that produces a Session, Goal, Dispatch, or Event.

### Outputs

- Either dispatch proceeds, or a structured refusal naming the unauthorized goal, the stream's authorized list, and the referenced ValueAction.
- Persisted artifacts carry `dec:inStream` linking to the active ValueStream (ADR-005).

### State

- The active ValueStream's authorized-goals set cached once at process start.
- A writer middleware adding `dec:inStream` to every applicable artifact insert.

### Behaviour

1. At process start, load the ValueStream from the store; cache authorized-goals.
2. On goal-driven command, look up the goal in the authorized set; refuse with structured message if absent.
3. On Session/Goal/Dispatch/Event writes, middleware ensures `dec:inStream` is present (defaults to active stream). If the caller supplies a *different* stream id, refuse with `ScopeError::ForeignStream`.

### Invariants

- After any successful Session/Goal/Dispatch/Event mutation, the triples include `dec:inStream <active-stream>` (ADR-005).
- An unauthorized goal never produces any persisted artifact — refusal happens before dispatch (ADR-005).
- Active stream id cannot change at runtime (ADR-012).

### Error handling

- Unauthorized goal → non-zero exit + "This stream pursues `<va-uri>`; `<goal>` is not authorized. Authorized: `<list>`."
- Foreign-stream insertion → `ScopeError::ForeignStream { active, supplied }`.
- Missing ValueStream at startup → `ScopeError::Uninitialized` pointing to `dec init`.

### Boundaries

- Does NOT define what goals exist globally — that's the ValueAction (FT-007).
- Does NOT implement role dispatch (FT-011).
- Speaks only of the four artifact classes named above; downstream artifacts (e.g., `CodeChange`) are scoped by their owning Session (ADR-004).

## Out of scope

- Multi-stream commands in one invocation.
- Cross-stream artifact import (per §3.6).
- Dynamic policy updates at runtime.
