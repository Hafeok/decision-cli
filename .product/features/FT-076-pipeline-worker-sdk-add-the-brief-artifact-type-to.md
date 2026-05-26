---
id: FT-076
title: 'pipeline-worker SDK: Add the Brief artifact type to product-cli''s catalog'
phase: 3
status: complete
depends-on:
- FT-070
- FT-072
adrs:
- ADR-044
tests:
- TC-141
domains: []
domains-acknowledged:
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-047: ADR-047 governs capability-tag binding via catalog at dispatch time. This feature does not perform capability-tag-to-entry binding.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-054: ADR-054 governs LiteLLM as the worker SDK's provider substrate. This feature does not call LiteLLM.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. Bootstrap Feature: required so that
Brief artifacts can be authored into the product-cli graph through `product author`.
Until it ships, working-session documents like the pipeline-worker-slice-1 Brief
remain as free-form markdown wrappers around the structured product-cli graph
(see `briefs/pipeline-worker-slice-1.md`).

Addresses ADR-044 (Brief as a typed artifact in product-cli's catalog).

Lands on top of the dual-provenance discipline (FT-069 mechanical-provenance
SHACL, FT-070 motivational-predicate vocabulary, FT-072 shipped shape files,
FT-073 GraphWriter enforcement, and ADR-038…ADR-041). The Brief shape conforms
to that discipline from day one — no provisional rules to reconcile later.

## Scope

- SHACL shape for `Brief` with fields: `title`, `premise`, `goal`,
  `success_criteria`. Conforms to:
  - Mechanical-provenance fragment from FT-069 (auto-attached
    `prov:wasGeneratedBy` / `prov:wasAttributedTo` / `prov:generatedAtTime`).
  - Motivational-predicate row added to FT-070's per-type vocabulary table for
    Brief (the Brief's own `references`/`motivated_by` predicate that points
    upstream to whatever motivated the Brief itself — typically an external
    document or a value-stream signal, so this Brief may land most Briefs as
    `BoundaryArtifact` per ADR-040 / FT-071).
- Brief edges:
  - `decomposes_into → Feature[]` — Features this Brief authorizes.
  - `excludes → Feature[]` — explicit non-goals tracked for re-promising
    detection.
  - `acknowledges → Acknowledgement[]` — recorded debts the Brief takes on.
  - `references → Artifact[]` — out-of-band references (docs, sibling Briefs,
    external standards).
- product-cli schema migration to add the type to the catalog.
- `product author` recognizes `## Brief <id>` sections in working-session files.
- Brief-aware queries:
  - "show me all Features decomposed from BRIEF-X"
  - "show me everything BRIEF-X excluded"
- Ship Brief's row into FT-072's shape-file bundle (the SHACL shape is
  distributed alongside Feature/ADR/TC/Dep shapes, not as a separate add-on).

## Out of scope

- Nested Briefs (the granularity question — defer until 2-3 Briefs have been
  authored and patterns emerge in practice).
- Brief versioning / supersession (defer until a real Brief needs revising).

## Success criteria

- `product brief new` and `product brief show` work end-to-end.
- The pipeline-worker-slice-1 Brief working document can be re-authored as a
  typed Brief artifact and its edges (`decomposes_into`, `excludes`, etc.)
  resolve against the existing FT-077…FT-085 Features.
- `product graph check` validates Brief artifacts under the new SHACL shape,
  including dual-provenance conformance via FT-073's GraphWriter enforcement.
- Brief participates in FT-075's full-chain provenance query as a first-class
  upstream node.

## Notes

- ID convention TBD: working session used `brief:<slug>`, product-cli convention
  may differ (`BRIEF-NNN`? IRIs under a base namespace?). Adopt whatever the
  existing convention is.
- Coordinate with FT-070 author: if FT-070 ships before this Feature, Brief's
  motivational-predicate row is added here as part of FT-076. If FT-070 has
  not yet enumerated Brief, this Feature extends FT-070's vocabulary table
  rather than authoring a parallel one.