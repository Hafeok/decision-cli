---
id: FT-174
title: 'decision-cli: cluster cells resolve tool surfaces from the role catalog with per-cell narrowing'
phase: 5
status: planned
depends-on:
- FT-170
adrs:
- ADR-088
- ADR-070
- ADR-080
tests:
- TC-441
- TC-442
- TC-443
- TC-444
domains:
- security
- workers
domains-acknowledged:
  ADR-082: Cell tools narrowing is TaskType-level substrate; archetype contracts are unaffected and inherit the rule unchanged.
  ADR-084: No archetype ships or changes status in this slice; seam audits are out of scope for the tool-surface computation.
  ADR-083: The tools declaration binds at the TaskType cell level by construction (dispatch parameter per ADR-083's litmus), introducing no archetype- or instance-bound detail.
  ADR-081: No enumerate/lookup verb pair is added or changed; the cell surface renders inside existing session show output.
  ADR-087: FT-174 touches the validation pass and payload construction, not audit emission; verdict consumption is FT-173's slice.
---

## Description

Implements [ADR-088](ADR-088): removes the hardcoded `allowed_tools: ["read_file", "write_file"]` in `cluster_dispatch.rs` and replaces it with the effective surface `role catalog surface ∩ cell narrowing`. The role surface comes from `role_catalog::lookup()` — the same path the standard dispatch flow uses under [ADR-070](ADR-070) — and a Cell may optionally declare a `tools` subset. Widening attempts and empty intersections fail the cluster run at validation, before any cell dispatches.

## Functional Specification

### Inputs

- The dispatching role's `dec:roleTool` set from the orchestration store (seeded per ADR-070).
- The TaskType cell catalog, extended with an optional per-cell `tools: [..]` declaration.
- The existing cluster validation pass ([FT-170](FT-170)) that runs before cell one dispatches.

### Outputs

- `DispatchPayload.allowed_tools` computed per cell as `role surface ∩ cell tools` (full role surface when the cell declares nothing); the `cluster_dispatch.rs:760` hardcode removed.
- Validation diagnostics: widening declaration → error naming cell, offending tool, and role surface; empty intersection → error naming cell and both sets.
- Cell `tools` declarations surfaced in the cluster SessionRecord so `dec session show` can explain a cell's surface.

### State

- No new artifact types. The cell `tools` field lives in the TaskType declaration (the FT-150 cell substrate when graph-resident; the in-binary catalog meanwhile).

### Behaviour

1. Cluster validation resolves the role surface once per run and computes each LLM-backed cell's effective surface.
2. Any widening declaration or empty effective surface aborts the run during validation — zero worker subprocesses, zero tokens.
3. Each dispatched cell's payload carries its effective surface; the worker enforces it unchanged (fail-closed per ADR-070).
4. Mechanical cells (no model binding) carry no surface and skip the computation.

### Invariants

- No code path constructs a cluster `DispatchPayload.allowed_tools` from a literal; the catalog is the only source.
- The effective surface is never a superset of the role surface.
- Repair-round re-dispatches ([FT-171](FT-171)) reuse the surface computed at validation — the surface is stable across rounds within one run.

### Error handling

- Missing role surface (legacy store without `dec:roleTool` quads) keeps ADR-070's grandfathering: empty surface → fail-closed validation error telling the operator to re-seed the catalog.
- Validation failures are operator-facing diagnostics with the cluster run aborted cleanly (sandbox untouched).

### Boundaries

- Worker-side enforcement is untouched (ADR-070 §4 governs it).
- Sub-resource scoping (constrain `write_file` to the cell's `output_path`) remains deferred per ADR-070/ADR-088.
- The standard (non-cluster) dispatch path is untouched — it already resolves from the catalog.

## Out of scope

- Per-cell role overrides (a cell dispatching under a different role than the cluster's).
- Narrowing for non-cluster dispatches (the role surface is already the right granularity there).
- Any change to the worker tool registry or its containment rules ([ADR-071](ADR-071)).
