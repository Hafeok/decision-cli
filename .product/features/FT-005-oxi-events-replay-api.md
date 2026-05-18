---
id: FT-005
title: 'oxi-events: Replay API'
phase: 1
status: planned
depends-on:
- FT-003
adrs:
- ADR-001
- ADR-002
- ADR-004
- ADR-008
- ADR-005
- ADR-012
tests:
- TC-009
domains: []
domains-acknowledged: {}
---

## Description

The replay API answers "give me events for capability X since seq N" via SPARQL over the events graph. Per **ADR-002 (Graph-as-state)** there is no separate event log — the graph is the durable event log, so replay is a query, not a side-channel. Stays within the oxi-events SDP boundary per **ADR-001**.

See `decision-cli-slice-1-bounds.md` §5.2, §5.4.

## Functional Specification

### Inputs

- A replay request: `{ since_seq: u64, until_seq: Option<u64>, filter: Option<SparqlFilterFragment>, limit: Option<usize> }`.
- A read handle to the Oxigraph store.

### Outputs

- An ordered iterator/stream of `Event` records, seq-ascending, optionally bounded by `until_seq` and `limit`.

### State

- None of its own.

### Behaviour

1. Translate the request into a SPARQL SELECT against the events named graph, projecting event fields and ordering by seq.
2. Apply the optional filter fragment as a `FILTER` clause.
3. Stream via oxigraph's streaming query API.

### Invariants

- Replay against an unchanged graph is deterministic.
- The output seq sequence is strictly increasing.
- Replay never observes `published = false` events with no corresponding mutation.

### Error handling

- Invalid filter SPARQL → `ReplayError::InvalidFilter` at request time.
- Store read errors → `ReplayError::Store(_)`.

### Boundaries

- Replay does NOT mutate event state.
- No cross-store federation.
- Consumer offsets are the consumer's responsibility (ADR-002).

## Out of scope

- Push-based replay (FT-004 already covers live-with-since-cursor).
- Replay rate-limiting / throttling.
- Snapshot/compaction interactions — slice 1 retains all events.
