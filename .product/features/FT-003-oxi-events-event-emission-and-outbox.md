---
id: FT-003
title: 'oxi-events: Event emission and outbox'
phase: 1
status: complete
depends-on:
- FT-001
- FT-002
adrs:
- ADR-001
- ADR-002
- ADR-004
- ADR-008
- ADR-005
- ADR-012
tests:
- TC-009
- TC-010
- TC-011
domains: []
domains-acknowledged: {}
---

## Description

Event emission turns `SubscriptionMatch` deltas (FT-002) into durable `Event` artifacts in the graph and publishes them through configured transports. Per **ADR-002 (Graph-as-state)** events live in the graph itself, not in a separate log. Per **ADR-004 (PROV-O)** every event links back via `prov:wasGeneratedBy` to its triggering mutation. The outbox flag plus a background publisher gives crash-safe at-least-once delivery.

See `decision-cli-slice-1-bounds.md` §5.2.

## Functional Specification

### Inputs

- A set of `SubscriptionMatch` records from FT-002 plus the mutation id and sequence number from FT-001.
- A configured list of delivery transports (in-process tokio broadcast here; SSE from FT-004).

### Outputs

- One `Event` per match, persisted as a quad set in the events named graph.
- Per-event delivery acks/nacks that flip the `published` flag.

### State

- The events named graph in Oxigraph.
- A background outbox publisher task that periodically scans for `published = false` events and retries delivery.
- An in-process tokio broadcast channel for co-located consumers.

### Behaviour

1. For each `SubscriptionMatch`, mint an `Event` with id, monotonic seq, subscription id, delta payload, `prov:wasGeneratedBy` link (ADR-004), `published = false`.
2. Persist atomically with the originating commit (or immediately after, depending on inline/async mode).
3. Publish to the broadcast channel; on confirmed send, flip `published = true`.
4. The outbox publisher resumes on startup via SPARQL: `?e WHERE { ?e a oxi:Event ; oxi:published false }`.

### Invariants

- An event's seq is strictly greater than any earlier event in the same store.
- `published = true` is set only after the transport confirms.
- An event's PROV-O chain resolves to an existing mutation (ADR-004).

### Error handling

- Broadcast send with no receivers is NOT an error in slice 1 (in-process bus is lossy fan-out; remote receivers use SSE).
- Outbox publisher errors are logged; event stays `published = false` for next sweep.
- Persistence failure → `EventError::Persist(_)` propagated to writer.

### Boundaries

- SSE delivery lives in FT-004; replay in FT-005.
- Event payload schema is opaque to oxi-events; consumers define their own shapes (ADR-001).

## Out of scope

- Exactly-once delivery across remote transports.
- Event compaction or retention policy.
- Cross-store event mirroring.
