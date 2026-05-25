---
id: ADR-048
title: Build-time SHACL codegen for typed worker Bundle accessors and Artifact builders
status: accepted
features:
- FT-079
- FT-080
- FT-085
supersedes: []
superseded-by: []
domains: []
scope: domain
content-hash: sha256:c4ccbae5a7280573c0e1cfa4d1355bbdb10013a9e10d2f0d2db3c8937b0d4e74
---

## Context

The SDK exposes typed surfaces (Bundle accessors FT-079, Artifact builders
FT-080) generated from SHACL shapes. Two ways to do the generation:

- **Build-time:** code generator runs ahead of release; generated modules
  are checked into the SDK repo; CI verifies they're up to date.
- **Runtime:** SDK reads shapes at import time and generates classes
  dynamically (similar to ORM model-introspection patterns).

## Decision

Build-time codegen. Generated modules are checked into the SDK repo. CI on
both the harness repo (pipeline-cli, where shapes live) and the SDK repo
runs the generator and fails on drift.

## Consequences

- **Positive:** IDE and type-checker friendly. Workers see real classes with
  real type hints; autocomplete works; mypy/pyright catches errors at edit
  time, not at session time.
- **Positive:** Predictable startup. No shape-parsing on every worker boot;
  cold-start latency stays low.
- **Positive:** SDK release boundary matches shape-version boundary. When
  the SDK ships version 0.4.0, the shapes it was generated against are an
  auditable artifact (the generated modules in git). Workers pinning an SDK
  version pin to a shape version.
- **Negative:** SDK must release when shapes change. Acceptable at current
  cadence (shapes are stable; the slice-1 worker SDK is the first consumer);
  revisit if shape churn becomes the bottleneck.
- **Negative:** Generated files in git create review noise. Mitigated by
  CI's drift check making them mostly mechanical to keep current.

## Alternatives considered

- **Runtime codegen.** Rejected: no IDE help, slower cold start, harder to
  audit "what shape was the SDK generated against?" The trade buys
  flexibility we're not asking for.
- **Hand-written typed surfaces.** Rejected: defeats the purpose. The whole
  point is to enforce that "what the harness packs is what the SDK exposes"
  — a hand-written surface is one more place semantic drift can hide.

## References

- `feature:shape-codegen` (FT-085) is the generator itself.
- `feature:bundle-layer` (FT-079) and `feature:artifact-layer` (FT-080)
  consume the generator's output.
- ADR-051 (artifact builder codegen alongside bundle accessors — same pipeline,
  two surfaces).