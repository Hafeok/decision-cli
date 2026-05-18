---
id: FT-001
title: 'oxi-events: GraphWriter mutation chokepoint'
phase: 1
status: planned
depends-on: []
adrs:
- ADR-001
- ADR-002
- ADR-004
- ADR-008
- ADR-005
- ADR-012
tests:
- TC-009
- TC-014
domains: []
domains-acknowledged: {}
---

## Description

`GraphWriter` is the mutation chokepoint over an Oxigraph store, per **ADR-002 (Graph-as-state)** and **ADR-001 (oxi-events SDP boundary)**. All graph writes from the orchestrator route through it. The writer mints sequence numbers, drives subscription evaluation (FT-002), emits events with PROV-O provenance (FT-003 + ADR-004), and exposes only framework vocabulary — mutations, subscriptions, events.

See also `decision-cli-slice-1-bounds.md` §5.1, §5.2.

## Functional Specification

### Inputs

- A handle to a configured `oxigraph` store (in-memory or on-disk).
- Mutation requests: typed structs describing a triple/quad set to insert/remove, an optional named-graph target, caller-supplied provenance metadata (actor, cause, timestamp source).
- A reference to the subscription registry (owned by the writer; populated by FT-002).

### Outputs

- A `CommitResult`: assigned mutation id, monotonic sequence number, affected named graphs, set of subscription matches re-evaluated, resulting event handles emitted (forwarded to FT-003 outbox).
- Errors as typed `thiserror` variants (see Error handling).

### State

- The Oxigraph store (owned externally; clonable handle).
- A monotonic sequence-number generator persisted as a graph triple (survives restart).
- The subscription registry handle.
- An in-process commit lock serialising writes so subscription evaluation sees a consistent post-commit snapshot.

### Behaviour

1. Accept a mutation request.
2. Apply the mutation inside a transaction.
3. On successful commit, mint the next sequence number and a mutation id.
4. Hand the post-commit snapshot to the subscription evaluator (FT-002); receive affected subscription set.
5. Emit one event per affected subscription (FT-003) with PROV-O links per ADR-004.
6. Return the `CommitResult`.

### Invariants

- Every successful mutation has exactly one mutation id and contiguous sequence-number issuance (no gaps).
- Events emitted by a single mutation share `prov:wasGeneratedBy` pointing to that mutation (ADR-004).
- A failed mutation produces no events and leaves no partial state.

### Error handling

- Store-level errors propagate as `WriterError::Store(_)` and abort commit.
- Subscription-evaluation errors are isolated per-subscription; a failing subscription records an event with `event:status = failed` so consumers see it.
- Sequence-number persistence failure aborts the commit.

### Boundaries

- Delivery transports live elsewhere (FT-003 in-process broadcast, FT-004 SSE).
- ADR-001 prohibits DDD vocabulary in the public API.
- Read paths bypass the writer entirely — only writes route through here.

## Out of scope

- Multi-store federation.
- Cross-process write coordination (single-process owner only).
- Schema/SHACL validation on the writer path — decision-cli enforces SHACL at the init/validation boundary (FT-008).
- Bulk-import optimisation paths.
