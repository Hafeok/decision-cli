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
domains-acknowledged: {}
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
