---
id: FT-076
title: 'pipeline-worker SDK: Add the Brief artifact type to product-cli''s catalog'
phase: 3
status: planned
depends-on:
- FT-070
- FT-072
adrs:
- ADR-044
tests: []
domains: []
domains-acknowledged: {}
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