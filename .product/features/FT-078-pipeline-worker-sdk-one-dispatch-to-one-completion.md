---
id: FT-078
title: 'pipeline-worker SDK: One dispatch to one completion lifecycle with in-memory pyoxigraph store'
phase: 3
status: complete
depends-on:
- FT-077
- FT-069
- FT-073
adrs:
- ADR-049
- ADR-050
tests:
- TC-143
domains: []
domains-acknowledged:
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. The Session is the unit of
measurement on the worker side and mirrors the harness's session record on the
other side of the wire. Addresses ADR-049 (pyoxigraph in-memory store) and
ADR-050 (Session IS a `prov:Activity`).

Depends on the dual-provenance discipline (FT-069 mechanical-provenance SHACL
and FT-073 GraphWriter enforcement, governed by ADR-038 and ADR-041): the
session record on the harness side becomes the `prov:Activity` whose URI this
in-process Session shares, and mechanical provenance triples on produced
artifacts are populated by the harness's GraphWriter — not by the worker.

## Location

`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/session.py` (and tests
under `workers/pipeline-worker-sdk/tests/`).

## Scope

- One `Session` object per dispatch, lifecycle bound to the dispatch lifetime.
- Owns an in-memory `pyoxigraph.Store` initialized from the dispatch's bundle
  N-Quads payload. The store holds the session's sub-graph for the duration of
  the call and is discarded on completion.
- Accumulates telemetry across all provider calls and side-channel emissions.
- On clean exit: serializes artifact triples + side-channel triples + telemetry
  into a completion payload (handed to FT-077 wire layer to post).
- On exception: emits whatever side-channel triples were captured and posts a
  `blocked` or `escalated` completion (no silent drops).
- The Session IS a `prov:Activity` (ADR-050). Mechanical provenance annotations
  on produced artifacts (`prov:wasGeneratedBy`, `prov:wasAttributedTo`,
  `prov:used`) are populated by the harness's GraphWriter from the session
  record at write time (FT-073 enforces SHACL, FT-069 ships the fragment) —
  the worker does not duplicate these on the wire.

## Out of scope

- Per-session persistence (sessions are ephemeral on the worker; the harness
  owns the durable session record).
- Multi-dispatch sessions / batching (one dispatch ⇒ one session).
- Mechanical-provenance triple emission from the worker side (owned by FT-069
  / FT-073 on the harness side).

## Success criteria

- A dispatch with N bundle triples produces a Session whose `pyoxigraph.Store`
  contains exactly those N triples on entry.
- On clean completion, the completion payload contains: artifact triples,
  side-channel triples (if any), and the telemetry block.
- On uncaught exception inside worker code, the SDK posts a `blocked`
  completion with the captured side-channel triples rather than dropping them.
- The Session's URI is the same URI used by the harness's `prov:Activity`
  record for that dispatch — verifiable by FT-075's full-chain provenance
  query.