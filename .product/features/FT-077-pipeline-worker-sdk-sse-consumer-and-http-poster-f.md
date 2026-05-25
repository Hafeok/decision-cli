---
id: FT-077
title: 'pipeline-worker SDK: SSE consumer and HTTP poster for the dispatch/completion protocol'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-045
- ADR-046
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. Implements the only layer of the
worker SDK that knows the network exists. Addresses ADR-045 (SSE for dispatches,
HTTP POST for completions) and ADR-046 (N-Quads on the wire).

## Location

`workers/pipeline-worker-sdk/` — sibling of `workers/code-writer/`. Slice 1's
first consumer of the SDK is the code-writer worker itself, which migrates
off its current hand-rolled bundle/artifact handling onto the SDK in a
follow-on slice.

## Scope

- Long-lived SSE connection to the harness's dispatch endpoint.
- Advertises the worker process's capability tags on connect.
- Resumes with `Last-Event-ID` on reconnect; replays missed dispatches.
- HTTP POST for completion events; retry on transient failures with backoff.
- Atomic claim requests on incoming dispatches (handles multi-worker capability-
  tag contention — first claimer wins, others move on).
- Model-catalog response cache per worker process (avoid re-fetching the
  capability-tag → model-group mapping on every dispatch).
- Surfaces dispatches to the Session layer (FT-078) via an async iterator.

## Out of scope

- WebSocket / NATS / any non-HTTP transport (rejected in ADR-045).
- JSON-LD payload conversion (ADR-046 commits to N-Quads at the boundary).
- Multi-tenancy / token rotation (deferred per `ack:security-deferred`).

## Success criteria

- A worker process subscribes, receives a dispatch event for its advertised
  capability tag, and posts a completion that the harness accepts with HTTP 200.
- Disconnect during a session resumes from the correct `Last-Event-ID` on
  reconnect.
- Concurrent claim attempts from two workers on the same dispatch resolve with
  exactly one winner.