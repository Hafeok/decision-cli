---
id: TC-145
title: 'pipeline-worker SDK: Typed artifact builders with SHACL validation at commit — exit criterion'
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-145-pipeline-worker-sdk-artifact-layer.sh
runner-timeout: 120
last-run: 2026-05-28T08:48:31.221315450+00:00
last-run-duration: 0.8s
---

## Description

Exit criterion for FT-080 (pipeline-worker SDK: Typed artifact builders
with SHACL validation at commit). Drives the artifact-builder pytest
suite under `workers/pipeline-worker-sdk/tests/test_tc_145_artifact_builder_commit.py`
against the in-memory pyoxigraph store.

The pytest suite covers the three success criteria the parent feature
names:

1. **Defensive SHACL conformance at commit.** A builder missing a
   required field — including a required motivational predicate from the
   per-type shape's `sh:or` (FT-072) — raises `CommitError` with a
   SHACL-derived message naming the shape, the focal IRI, and every
   alternative that would satisfy the constraint, before any wire send.
   Boundary-originated artifacts satisfy the `sh:or` via
   `mark_boundary_artifact(external_origin=…)` per ADR-040.
2. **No semantic drift with the harness.** Locally validated triples
   conform to the same SHACL shape (`workers/_shared/shapes/*.ttl`) the
   harness's GraphWriter re-validates on receive (FT-073 / ADR-041), so
   the local pyshacl-tier defensive check and the authoritative
   oxigraph-shacl check agree by construction. Workers do **not** emit
   mechanical provenance (`prov:wasGeneratedBy`,
   `prov:wasAttributedTo`, `prov:generatedAtTime`) — the harness
   materialises those at write time per ADR-050 / FT-069.
3. **Escape-hatch telemetry.** `builder.emit_triple(s, p, o)` increments
   a per-builder counter that aggregates onto the session's
   `artifact_escape_hatch_count` and surfaces in the completion event's
   telemetry block — the same gap-surface pattern FT-079 uses for
   `bundle.raw_store` accesses.

The suite also exercises the `Session.commit_artifact(builder)` wiring,
the multi-commit guard, and class-level shape-metadata introspection so
the harness and downstream tools can audit builder fitness without
instantiating one.

A passing run means every builder under
`pipeline_worker_sdk.artifact._generated` derives its validation
behaviour from the same SHACL shapes the harness loads, and the typed
write-side surface stays in sync with the read-side accessors generated
by the same FT-085 codegen pipeline.