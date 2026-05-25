---
id: FT-079
title: 'pipeline-worker SDK: Curated query helpers over the in-memory bundle sub-graph'
phase: 3
status: complete
depends-on:
- FT-078
- FT-085
adrs:
- ADR-048
- ADR-013
- ADR-016
tests:
- TC-144
domains: []
domains-acknowledged:
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. Wraps the session's in-memory
pyoxigraph store with role-specific typed accessors generated from the role's
bundle SHACL shape. Workers must not write SPARQL by hand — that's how
semantic drift between sides of the wire begins. Addresses ADR-048 (build-time
SHACL codegen).

## Location

`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/bundle/` — the generated
typed-accessor modules are checked in here, one module per role bundle shape.
The hand-written `Bundle` facade that exposes them sits at
`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/bundle/__init__.py`.

## Scope

- Generated typed accessors per role bundle shape:
  - `bundle.focal()` — the artifact under work
  - `bundle.linked_adrs()` — ADRs that govern the focal artifact
  - `bundle.applicable_test_criteria()` — TCs the action must satisfy
  - …and any other role-specific accessor declared in the bundle shape
- Accessors are deterministic and idempotent: same store + same query ⇒
  same return.
- `bundle.raw_store` escape hatch for diagnostic and shape-uncovered cases.
  Use of `raw_store` is flagged in telemetry as a gap-surface signal — patterns
  that recur become candidates for codegen extension.

## Out of scope

- Hand-written role-specific accessors (those are codegen outputs from
  FT-085, not authored in this slice).
- Mutation through the bundle (read-only; writes go through FT-080 Artifact
  builders).

## Success criteria

- A role's worker calls `bundle.focal()` and gets a typed Python object that
  matches the shape declared by the bundle SHACL.
- Calls to `bundle.raw_store` increment a telemetry counter that surfaces on
  the completion event.
- Two workers calling the same accessor on the same bundle store return
  byte-identical results.