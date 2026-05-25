---
id: FT-085
title: 'pipeline-worker SDK: Build-time codegen of typed Bundle and Artifact surfaces from SHACL'
phase: 3
status: complete
depends-on: []
adrs:
- ADR-048
- ADR-013
- ADR-016
tests:
- TC-150
domains: []
domains-acknowledged:
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. Reads SHACL shapes from
`pipeline-cli/schemas/` (which after FT-072 includes per-type shape files for
Feature, ADR, TC, Dep, and — once FT-076 lands — Brief, all with the
dual-provenance fragments) and generates the typed Bundle accessors (FT-079)
and Artifact builders (FT-080). Output is checked in. Addresses ADR-048
(build-time codegen).

This is the feature that enforces the shared-shape principle: what the
harness packs is what the SDK exposes, with no semantic drift between sides.

## Location

The codegen tool itself: `workers/pipeline-worker-sdk/tools/codegen/` (Python
script invoked via `uv run codegen`).

Generated outputs:
- `workers/pipeline-worker-sdk/src/pipeline_worker_sdk/bundle/_generated/`
- `workers/pipeline-worker-sdk/src/pipeline_worker_sdk/artifact/_generated/`
- `workers/pipeline-worker-sdk/src/pipeline_worker_sdk/schemas/_generated/`
  (Pydantic models for structured-output schemas)

All `_generated/` directories carry a header banner and are listed in
`.gitattributes` as `linguist-generated` to keep diffs from polluting
review.

## Scope

- A codegen tool (Python script invoked via `uv run` from the SDK
  package) that:
  - Reads the SHACL shapes from a configured directory (the harness's
    `schemas/` is the source of truth, exact path passed by env or CLI flag).
  - Emits typed Python modules per role: bundle accessors + artifact
    builders + Pydantic models for structured-output schemas.
  - Output checked into the SDK repo under `_generated/` subpackages.
- CI on both repos (pipeline-cli and the worker SDK) runs codegen and fails
  on drift — the generated files in the SDK must match what's produced from
  the harness's current shapes.
- A `justfile` / `pyproject.toml` script target so authors can regenerate
  locally before commit (`uv run codegen` or `just codegen`).

## Out of scope

- Runtime shape loading (rejected in ADR-048).
- Per-shape custom templates (one generator, parameterized by shape;
  per-role customization is achieved through the shape, not the generator).

## Success criteria

- A shape change in the harness, propagated through the codegen pipeline,
  produces a diff in the SDK's generated modules; CI fails until that diff is
  committed.
- The generated modules compile (import cleanly) and pass type checking
  (mypy/pyright) without hand-edits.
- The codegen output is byte-stable: running the generator twice in a row
  produces no diff.