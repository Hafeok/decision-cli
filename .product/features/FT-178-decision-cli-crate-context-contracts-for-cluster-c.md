---
id: FT-178
title: 'decision-cli: crate-context contracts for cluster cells — dependency universe + distilled real interfaces in every LLM cell bundle'
phase: 4
status: complete
depends-on:
- FT-177
adrs:
- ADR-091
tests:
- TC-457
- TC-458
- TC-459
- TC-460
domains: []
domains-acknowledged: {}
---

## Description

Companion slice to [FT-177](FT-177) under [ADR-091](ADR-091), closing the last witnessed context gap from the FT-148 cluster runs. With SPMC bundles the cells converge (run 10/11: all cells complete, audit reached), but the compile probe's error census is one defect class: `use oxigraph::…` (the crate only has `oxrdf`), `crate::ontology::application_contract::Provenance` (it lives at `crate::ontology::provenance`), invented `shapes` modules — **the cells were never told which crate they are writing into.**

Two additions to the TaskType declaration, both deterministic:

1. **Crate contract** — a fixed text block on the TaskType naming the target crate, its allowed dependency universe, and its forbidden crates (for `add-artifact-type`: `oxrdf`/`thiserror`/`serde`/`chrono`/`uuid`; never `oxigraph`/`tokio`/`std::fs`).
2. **Context files** — repo paths whose *distilled public surface* (FT-177's distiller, run at dispatch time against the live tree) is appended to every LLM cell bundle as "Existing crate interfaces — use exactly these" (for `add-artifact-type`: `crates/dec-ontology/src/ontology/provenance.rs`).

## Functional Specification

### Inputs

- `TaskTypeDecl` in `crates/dec-harness/src/task_type/types.rs`; the `add-artifact-type` registry entry.
- `build_cell_bundle` and `distill_rust_public_surface` (FT-177) in `cluster_dispatch.rs`.

### Outputs

- `TaskTypeDecl` gains `crate_contract: String` (empty ≡ none) and `context_files: Vec<PathBuf>` (repo-relative; missing files warn, never fail dispatch).
- `build_cell_bundle` renders, for every LLM cell: the crate contract verbatim, then each context file's distilled surface under an "Existing crate interfaces" heading.
- `add-artifact-type` declares both; other TaskTypes default to empty (unchanged).

### State

- None graph-resident.

### Behaviour

1. Context files are read and distilled per dispatch from the live workdir — the bundle always reflects the real current interface.
2. Mechanical cells (no LLM) skip both sections.

### Invariants

- An `add-artifact-type` LLM cell bundle names `oxrdf` as the model vocabulary and forbids `oxigraph` explicitly.
- The bundle's `Provenance` surface is byte-derived from the live `provenance.rs` — no hand-maintained copy to drift.

### Error handling

- Unreadable context file → `tracing::warn!` + section omitted; dispatch proceeds.

### Boundaries

- Distillation stays the FT-177 function; no new extraction logic.
- Other TaskTypes adopt the fields in their own slices.

## Out of scope

- Example-by-pattern context (e.g. distilled archetype module) — add only if the witnessed runs show interface context alone is insufficient.