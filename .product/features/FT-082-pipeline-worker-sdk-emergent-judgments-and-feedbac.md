---
id: FT-082
title: 'pipeline-worker SDK: Emergent judgments and feedback emission via side-channel'
phase: 3
status: complete
depends-on:
- FT-078
adrs: []
tests:
- TC-147
domains: []
domains-acknowledged:
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. Implements the two side-channel
APIs from `docs/ddd/Implementing_DDD.md` §6: emergent judgment recording (for
in-authority calls the worker makes during execution) and feedback emission
(for out-of-authority issues that need to escalate upstream).

## Location

`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/side_channel/` — both
APIs are exposed as methods on `Session` (from FT-078) but the emission/
packaging logic lives in this module.

## Scope

- `session.record_emergent_judgment(decision, rationale)`:
  - For in-authority judgments the worker makes mid-session.
  - Triples land in the artifact's metadata.
  - Surfaced to the paired interpretation session (FT-019…FT-025 verifier
    pipeline) for review.
- `session.emit_feedback(class, severity, evidence, blocking=False)`:
  - Emits a `Feedback` artifact conforming to the feedback schema
    (FT-026 / ADR-022).
  - `class` drawn from the controlled vocabulary (ADR-023).
  - `blocking=True` causes the session to exit with `outcome=blocked` (per
    ADR-025); non-blocking feedback flows alongside `outcome=completed`.
- Both APIs emit triples into the session's emission set, packaged into the
  completion event alongside the main artifact (no separate transport).

## Out of scope

- Persisting feedback locally on the worker (the harness owns durable
  feedback state — see FT-027 / FT-029).
- Routing decisions (the harness routes per ADR-026; worker only emits).

## Success criteria

- A worker calling `record_emergent_judgment` produces triples visible in the
  paired interpretation session's bundle.
- A worker calling `emit_feedback(blocking=True)` ends the session with
  `outcome=blocked` and the feedback artifact is included in the completion
  payload.
- A worker calling `emit_feedback(blocking=False)` does not affect session
  outcome but produces a Feedback artifact in the completion.