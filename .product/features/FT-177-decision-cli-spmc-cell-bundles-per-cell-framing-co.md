---
id: FT-177
title: 'decision-cli: SPMC cell bundles — per-cell framing contracts, distilled upstream context, and split test cells for add-artifact-type'
phase: 4
status: complete
depends-on:
- FT-170
- FT-171
adrs:
- ADR-080
- ADR-037
- ADR-091
tests:
- TC-453
- TC-454
- TC-455
- TC-456
domains: []
domains-acknowledged: {}
---

## Description

Applies the project's core context principle — *hallucination means the context was too big or not specific enough* — to the cluster's own bundles, after nine witnessed FT-148 runs isolated every remaining failure to the two heavy cells. The evidence is exact: the `parser` cell hallucinated vocab files for `Convention`/`TaskType`/`SeamAudit` — types mentioned only in the feature-spec prose it never needed; `round_trip_tests` consumed 1.25M input tokens (FT-147) carrying five full upstream artifact bodies when a test writer needs interfaces. Escalating to a bigger model is rejected — the funnel ([ADR-037](ADR-037)) reserves escalation for genuine reasoning depth, not for compensating over-fed bundles.

Three changes, all to the `add-artifact-type` TaskType ([FT-141](FT-141)) and the bundle builder ([ADR-080](ADR-080)):

1. **Per-cell framing contracts.** Only `rust_struct` receives spec framing, and only the `### Outputs` section (the shape it transcribes). Every downstream cell gets a one-line feature identity: its truth is the upstream artifacts, not the prose.
2. **Distilled upstream context (SPMC — single-purpose minimal context).** Cells flag whether upstream `.rs` content arrives whole or distilled to its public surface (struct/enum/const declarations and `fn` signatures, extracted deterministically — no LLM). Turtle stays whole (it is small and is itself an interface).
3. **Split the oversized test cell.** `round_trip_tests` becomes two cells with separate outputs and narrow bundles: `round_trip_test` (positive emit→validate→parse case) and `shacl_negative_tests` (the rejection cases).

## Functional Specification

### Inputs

- `CellDecl` / TaskType registry in `crates/dec-harness/src/task_type/` (FT-139/FT-166 shape).
- `build_cell_bundle` and the framing loader in `crates/decision-cli/src/features/drive/cluster_dispatch.rs` (FT-163 framing cap, FT-165 workflow block).
- The `add-artifact-type` audit (`scripts/checks/cluster-audit-add-artifact-type.py`) and its FT-172 compile probe's module auto-wiring.

### Outputs

- `CellDecl` gains `framing: CellFraming` (`SpecOutputs` | `Minimal`) and `distill_upstream: bool`. Existing TaskTypes default to today's behaviour (`SpecOutputs`-equivalent full framing, `distill_upstream: false`) so add-cli-subcommand and the worker clusters are untouched.
- A deterministic `distill_rust_public_surface(&str) -> String` in the dispatcher: keeps doc-comment-free `pub struct`/`pub enum` blocks (with bodies — fields are interface), `pub const` lines, and `pub fn` signatures (no bodies). Pure text processing; unit-tested.
- `add-artifact-type` cells re-declared: `rust_struct` (framing `SpecOutputs`, no upstream), `shacl_shape`/`iri_module_consts`/`parser`/`emitter` (framing `Minimal`, distilled upstream), and the split `round_trip_test` + `shacl_negative_tests` (framing `Minimal`, distilled upstream + full `.ttl`), with `output_path`s `…/{artifact_name}/tests.rs` and `…/{artifact_name}/negative_tests.rs`.
- FT-172 compile-probe module wiring treats any cell file named `tests*` / `negative_tests*` as `#[cfg(test)]` modules.

### State

- No graph-resident changes. `.dec/task-types.toml` routing unchanged.

### Behaviour

1. `build_cell_bundle` renders framing per the cell's `CellFraming`: `SpecOutputs` slices the spec body from the `### Outputs` heading to the next same-level heading (falling back to the FT-163 capped body when the section is absent); `Minimal` renders one line: feature id + title.
2. Upstream sections render distilled when `distill_upstream` is set, with a header noting "public surface only — the full body exists on disk".
3. The split test cells derive from `parser` + `emitter` (signatures) + `shacl_shape` (full ttl) + `rust_struct` (distilled).

### Invariants

- A `Minimal`-framed cell's bundle never contains feature-spec body text (the witnessed hallucination source).
- Distillation is deterministic: same input → same output; no LLM in the path.
- Bundle size for the former heavy cells drops by ≥80% against the FT-148 run-9 bundles (assert order-of-magnitude in tests via the fixture spec).

### Error handling

- A missing `### Outputs` section degrades to the existing capped-body framing with a `tracing::warn!` — never a dispatch failure.

### Boundaries

- Other TaskTypes adopt SPMC fields in their own slices; this slice changes only `add-artifact-type`.
- Worker contract unchanged (ADR-008) — this is bundle composition, all harness-side.

## Out of scope

- Capability escalation for cells (rejected here per the funnel; remains available as a separate lever if SPMC proves insufficient).
- Graph-resident TaskType artifacts ([FT-150](FT-150)) — these fields migrate with it.