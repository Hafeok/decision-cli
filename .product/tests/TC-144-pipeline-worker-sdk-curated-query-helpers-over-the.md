---
id: TC-144
title: 'pipeline-worker SDK: Curated query helpers over the in-memory bundle sub-graph — exit criterion'
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-144-pipeline-worker-sdk-bundle-layer.sh
runner-timeout: 120
last-run: 2026-05-25T23:43:32.181201737+00:00
last-run-duration: 0.7s
---

## Purpose

Exit criterion for [FT-079](FT-079): the worker SDK's curated bundle facade
exposes typed accessors per the role's bundle SHACL shape, accessors are
deterministic / idempotent across calls and across worker processes, and the
``raw_store`` escape hatch trips a session-level telemetry counter that
surfaces on the completion event.

## Given

A `pyoxigraph.Store` loaded with a curated bundle sub-graph containing:

- one focal `dec:Feature` with `dec:decomposesFrom` → a `dec:Brief`,
- two `dec:ADR` artifacts whose `dec:decidesFor` targets the focal,
- one `dec:ADR` whose `dec:decidesFor` targets a different feature,
- two `dec:TC` artifacts whose `dec:validates` targets the focal,
- one `dec:TC` validating a different feature.

A `Bundle` is built either directly via `Bundle(store, focal_iri)` or via the
session factory `Session(dispatch).bundle(focal_iri)`.

## When

```bash
pytest workers/pipeline-worker-sdk/tests/test_tc_144_bundle_layer.py
```

## Then

1. `bundle.focal()` returns a typed Python object — `FeatureAccessor` for the
   focal `dec:Feature` — whose populated fields agree with the codegen output
   generated from the bundle SHACL. An unknown focal type raises
   `UnknownFocalTypeError`.
2. `bundle.linked_adrs()` returns only the two ADRs whose `dec:decidesFor`
   targets the focal, in lexicographic IRI order.
3. `bundle.applicable_test_criteria()` returns only the two TCs whose
   `dec:validates` targets the focal, in lexicographic IRI order.
4. Two independent `Bundle` instances over the same store, and two
   independent stores loaded from the same N-Quads, both return
   byte-identical accessors (equality + `repr()`) for every curated method —
   the cross-worker determinism contract from FT-079 success criteria.
5. Each access to `bundle.raw_store` increments `bundle.raw_store_access_count`
   and, when the bundle was minted via `session.bundle(focal_iri)`, also
   increments `session.raw_store_access_count`. Curated accessor calls do
   NOT increment either counter.
6. `session.build_completion().telemetry["bundle_raw_store_access_count"]`
   equals the aggregate of raw-store accesses across every `Bundle` minted
   by the session — gap-surface signal is per-session, not per-bundle.

## Notes

- The codegen pipeline (FT-085) is exercised by TC-150; this TC validates
  the hand-written facade and the session telemetry handoff.
- The accessor dataclasses imported via `pipeline_worker_sdk.bundle.accessors`
  are the same generated modules workers consume in production, so type
  identity in the assertions is the same identity the worker would see.
- Raw-store access is intentionally cheap (a property + a counter bump) so
  the discouragement signal is observability, not friction.