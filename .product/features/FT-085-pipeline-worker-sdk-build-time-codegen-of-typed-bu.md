---
id: FT-085
title: 'pipeline-worker SDK: Build-time codegen of typed Bundle and Artifact surfaces from SHACL'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-048
tests: []
domains: []
domains-acknowledged: {}
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