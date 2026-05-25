---
id: ADR-045
title: 'Wire protocol: SSE for dispatch, HTTP POST for completion'
status: accepted
features:
- FT-077
supersedes: []
superseded-by: []
domains: []
scope: domain
content-hash: sha256:99a717209d5b8add8bfdd73ce813c6eaee827774d384fc0dabc5372f52ce0b5b
---

## Context

The pipeline-worker SDK needs a wire protocol between the harness
(pipeline-cli, Rust) and worker processes (Python, slice 1; potentially Rust
later). Two structurally different operations share the boundary:

- **Dispatch** (harness → worker): a broadcast event stream with replay
  semantics. The worker subscribes; the harness publishes when work matches
  the worker's capability tags; the worker must be able to resume from a
  known event ID on reconnect without losing work.
- **Completion** (worker → harness): a validated RPC submission with
  synchronous response semantics. The worker posts; the harness validates
  SHACL conformance, writes through GraphWriter, and returns success/failure.

These have different shape, different reliability requirements, and different
operational characteristics. Forcing them into one protocol over-couples
them.

## Decision

- **Dispatch:** Server-Sent Events (SSE) from the harness. Long-lived HTTP
  GET, `text/event-stream`, `Last-Event-ID` header for resume.
- **Completion:** HTTP POST from the worker to a per-session endpoint.
  Returns 2xx on accepted (SHACL passed, write committed) or 4xx with the
  validation report on rejected.

## Consequences

- **Positive:** Two well-understood protocols, each chosen for its native
  shape. SSE has standard replay semantics (`Last-Event-ID`), HTTP POST has
  standard request/response correlation.
- **Positive:** Standard HTTP infrastructure (load balancers, reverse
  proxies, observability) works out of the box for both directions.
- **Positive:** Worker SDK and harness can evolve their respective ends
  independently — adding a field to the dispatch event is an SSE evolution,
  not a protocol redesign.
- **Negative:** Two protocols to operate instead of one. Mitigated by both
  being HTTP-based and sharing the same auth/observability infrastructure.

## Alternatives considered

- **WebSocket for both directions.** Rejected: gains a single connection but
  re-invents replay semantics, request/response correlation, and HTTP
  back-pressure inside a custom frame protocol. Doesn't earn the
  complexity for slice 1's workload.
- **NATS bidirectional pub/sub.** Deferred: the right escalation if push
  latency or message rate ever exceeds what SSE+POST can support. State
  stays in Oxigraph; NATS would only carry wake-up signals. Revisit when
  there's a measured reason to.

## References

- `feature:wire-layer` (FT-077) implements this protocol on the worker side.
- `docs/ddd/Implementing_DDD.md` §7 (event substrate) on the harness side.