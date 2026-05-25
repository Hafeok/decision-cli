---
id: FT-083
title: 'pipeline-worker SDK: Driver abstraction for production and replay'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-013
- ADR-016
tests:
- TC-148
domains: []
domains-acknowledged:
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. Defines the `Driver` interface
that both EventDriver (FT-084, production) and ReplayDriver (slice 2, offline
replay) implement. Workers consume sessions via `async for session in driver:`
and never know which driver invoked them.

This is what operationalizes "per-role queries are the unit of evolution"
(`docs/ddd/Implementing_DDD.md` §4) — the same worker code runs against
historical bundles offline as runs against live dispatches.

## Location

`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/driver/` —
`base.py` for the protocol/ABC, `fake.py` for the in-memory test double.

## Scope

- The `Driver` protocol/ABC:
  - `__aiter__` / `__anext__` yielding `Session` objects
  - lifecycle hooks for clean shutdown
  - completion handoff (the worker calls back through the driver to post)
- Test doubles: an in-memory `FakeDriver` for SDK unit tests, accepting a
  pre-built list of `(bundle, expected_completion)` tuples.
- Documentation of the contract: what is and isn't observable to worker code
  about which driver is in use.

## Out of scope

- ReplayDriver implementation (slice 2 work).
- Concrete EventDriver implementation (split out as FT-084 so this Feature
  keeps a clean interface-only boundary).

## Success criteria

- A worker written against `Driver` runs unchanged under the FakeDriver in
  SDK tests and under EventDriver in integration.
- Type-checker rejects worker code that branches on driver implementation
  type.