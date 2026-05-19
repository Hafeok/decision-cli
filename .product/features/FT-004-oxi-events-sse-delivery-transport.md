---
id: FT-004
title: 'oxi-events: SSE delivery transport'
phase: 1
status: complete
depends-on:
- FT-003
adrs:
- ADR-001
- ADR-002
- ADR-008
- ADR-004
- ADR-005
- ADR-012
tests:
- TC-011
domains: []
domains-acknowledged:
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-004 has no feedback to gate.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-004 produces no feedback artifacts.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-004 produces no feedback artifacts.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-004's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-004 produces no action/interpretation pair.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-004 does not introduce or modify a role catalog entry.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-004 does not author or modify a fitness-function artifact.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-004 neither emits nor routes feedback.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-004's code is reorganised under that migration, not by this feature.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-004 is out of scope for the pairing.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-004 neither emits nor consumes verdicts.
---

## Description

The SSE (Server-Sent Events) delivery transport exposes the FT-003 event stream to remote consumers over HTTP. It serves the same logical stream as the in-process broadcast: live events plus optional from-seq replay (delegating replay semantics to FT-005). Stays within the oxi-events SDP boundary per **ADR-001**.

See `decision-cli-slice-1-bounds.md` §5.2.

## Functional Specification

### Inputs

- HTTP requests on the configured event-stream endpoint, optionally with `Last-Event-ID` header or `?since=<seq>`.
- A subscription handle on the in-process broadcast channel from FT-003.
- A reference to the events graph for replay (delegated to FT-005).

### Outputs

- An SSE stream framed per the W3C spec: `id: <seq>`, `event: <type>`, `data: <json payload>`.
- Standard HTTP error responses for malformed requests.

### State

- An axum HTTP router with a single event-stream route.
- A per-connection task fanning out from the broadcast channel.
- Optional per-connection replay cursor when `since` is provided.

### Behaviour

1. On connect with `since`/`Last-Event-ID`, stream historic events from the events graph in seq order until caught up to the live cursor.
2. Switch to live mode, forwarding events from the broadcast channel.
3. On client disconnect, drop the per-connection task; no lingering server-side state.
4. Heartbeat at a configurable interval.

### Invariants

- A client with `since: N` receives every event with seq > N exactly once (no duplication at the historic/live boundary).
- Events arrive in monotonic seq order per connection.
- The transport never mutates the graph.

### Error handling

- Malformed `since` → HTTP 400 with a structured error body.
- Slow-client backpressure → drop the connection after a bounded buffer fills.
- Server shutdown closes all SSE connections cleanly.

### Boundaries

- Event payloads are FT-003's responsibility, unchanged.
- No authentication or authorisation — slice 1 binds to localhost only.
- WebSocket transport explicitly out of scope.

## Out of scope

- Authentication, TLS termination, multi-tenant isolation.
- Custom topic routing (consumers filter client-side).
- Compression beyond HTTP-layer defaults.
