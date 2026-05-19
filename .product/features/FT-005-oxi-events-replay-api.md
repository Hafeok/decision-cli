---
id: FT-005
title: 'oxi-events: Replay API'
phase: 1
status: complete
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
domains-acknowledged:
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-005 does not introduce or modify a role catalog entry.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-005 does not author or modify a fitness-function artifact.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-005 has no feedback to gate.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-005 neither emits nor routes feedback.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-005 produces no feedback artifacts.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-005's code is reorganised under that migration, not by this feature.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-005 is out of scope for the pairing.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-005 produces no feedback artifacts.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-005 produces no action/interpretation pair.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-005 neither emits nor consumes verdicts.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-005's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
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
