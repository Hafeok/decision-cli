---
id: TC-128
title: WorkerImage artifact validates and is discoverable by capability tag
type: exit-criteria
status: passing
validates:
  features:
  - FT-086
  adrs:
  - ADR-055
phase: 3
runner: cargo-test
runner-args: -p decision-cli --test tc_128_workerimage_artifact_validates_and_is_discoverable
runner-timeout: 120
last-run: 2026-05-25T23:43:40.429452005+00:00
last-run-duration: 0.2s
---

## Purpose

Exit criterion for [FT-086](FT-086) (WorkerImage artifact in the
orchestration catalog) and validation of [ADR-055](ADR-055)
(WorkerImage mirrors the Model catalog).

## Given

- A fresh Oxigraph store seeded only with the runtime ontology.
- One or more `dec:WorkerImage` artifacts constructed in memory and
  inserted into the worker-image named graph via the published
  `to_quads` serialiser.

## When

```bash
cargo test -p decision-cli --test tc_128_workerimage_artifact_validates_and_is_discoverable
```

## Then

1. A well-formed `dec:WorkerImage` admits via the FT-086 SHACL
   validator (`validate_quads`) and round-trips through RDF back to
   an equal in-memory struct via `query_by_id`.
2. `query_by_capability_tag` returns every image that claims the tag
   and only those images; order is `(id ascending, version ascending)`.
3. `query_by_eligibility_status` partitions images by lifecycle state
   — a `qualified` image surfaces only under `Qualified`, a `pulled`
   image only under `Pulled`.
4. The composite query "find qualified images claiming capability tag
   X" — the success-criteria scenario named on FT-086 — resolves to
   the expected subset.
5. SHACL rejects images missing required fields (no `@sha256:` digest
   on `dec:registry_ref`, zero `dec:capability_tag` entries, unknown
   `dec:eligibility_status`).

## Notes

The test owns its own in-memory store and constructs artifacts via the
public API (`WorkerImage::to_quads`, `Store::insert`); no graph mutations
escape the test scope. The integration test mirrors the unit tests in
`crates/decision-cli/src/core/ontology/worker_image/tests.rs` but
exercises the discovery API end-to-end against a real `oxigraph::Store`.

## Formal specification

⟦Σ:Types⟧{
  Image ≜ dec:WorkerImage
  Tag ≜ String
  Status ≜ {qualified, candidate, deprecated, pulled}
}

⟦Γ:Invariants⟧{
  ∀ i:Image, t:Tag:
    t ∈ i.capability_tags ⇒ i ∈ query_by_capability_tag(store, t)
  ∀ i:Image:
    i ∈ query_by_eligibility_status(store, i.eligibility_status)
  ∀ i:Image:
    well_formed(i) ⇔ validate_quads(i.to_quads(...)) = Ok
}