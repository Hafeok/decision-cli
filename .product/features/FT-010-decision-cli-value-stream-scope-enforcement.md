---
id: FT-010
title: 'decision-cli: Value stream scope enforcement'
phase: 1
status: planned
depends-on:
- FT-008
- FT-009
adrs:
- ADR-005
- ADR-012
- ADR-002
- ADR-004
- ADR-008
- ADR-001
tests: []
domains: []
domains-acknowledged: {}
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
