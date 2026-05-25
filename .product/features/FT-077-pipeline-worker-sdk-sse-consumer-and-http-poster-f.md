---
id: FT-077
title: 'pipeline-worker SDK: SSE consumer and HTTP poster for the dispatch/completion protocol'
phase: 3
status: complete
depends-on: []
adrs:
- ADR-045
- ADR-046
- ADR-013
- ADR-016
tests:
- TC-142
domains: []
domains-acknowledged:
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
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