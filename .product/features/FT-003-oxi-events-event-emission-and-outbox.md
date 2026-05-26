---
id: FT-003
title: 'oxi-events: Event emission and outbox'
phase: 1
status: complete
depends-on:
- FT-001
- FT-002
adrs:
- ADR-008
tests:
- TC-009
- TC-010
- TC-011
domains: []
domains-acknowledged:
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-003 has no feedback to gate.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-003 produces no feedback artifacts.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-003 does not introduce or modify a role catalog entry.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-003's code is reorganised under that migration, not by this feature.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-003 does not author or modify a fitness-function artifact.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-003 produces no feedback artifacts.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-003 neither emits nor routes feedback.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-003 is out of scope for the pairing.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-003's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-003 produces no action/interpretation pair.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-003 neither emits nor consumes verdicts.
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
