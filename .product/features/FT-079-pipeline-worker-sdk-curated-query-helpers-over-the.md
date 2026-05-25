---
id: FT-079
title: 'pipeline-worker SDK: Curated query helpers over the in-memory bundle sub-graph'
phase: 3
status: planned
depends-on:
- FT-078
- FT-085
adrs:
- ADR-048
tests: []
domains: []
domains-acknowledged: {}
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