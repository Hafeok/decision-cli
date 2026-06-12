---
id: ADR-088
title: Cluster cells resolve their tool surface from the role catalog; cells may narrow it, never widen it
status: accepted
features: []
supersedes: []
superseded-by: []
domains:
- security
- workers
scope: domain
content-hash: sha256:2406fe8be3f4628fd7750a1468090f6a4096792f898655a8ad59951e1510ace0
---

**Status:** Proposed

## Context

[ADR-070](ADR-070) settled where a dispatch's tool surface comes from: the role catalog. Every `dec:Role` declares `dec:roleTool` literals; the dispatcher reads the surface via `role_catalog::lookup()` and threads it through `DispatchPayload.allowed_tools`; the worker fail-closes on the deny-list complement. That path is live for the standard implement/dispatch flow.

Cluster dispatch ([FT-139](FT-139) and successors) bypasses it. `cluster_dispatch.rs` builds its `DispatchPayload` directly and hardcodes the surface for every LLM-backed cell:

```rust
allowed_tools: vec!["read_file".to_string(), "write_file".to_string()],  // cluster_dispatch.rs:760
```

Two defects follow:

1. **Catalog drift.** The cluster path does not consult the catalog at all. If an operator narrows the implementer role's surface in the graph (say, removes `write_file` from a role that should only propose), cluster cells keep writing. If the catalog widens (a cell legitimately needs `run_tests` to self-check), the hardcode wins and the capability silently never reaches the model. The graph is supposed to be the source of truth ([ADR-036](ADR-036)); here a Rust literal is.
2. **No per-cell discipline.** Every cell gets the identical surface, although cells are *designed* to be narrower than a broad dispatch — that is the entire point of the decomposition under [ADR-080](ADR-080). An `iri_module_consts` emitter that must produce exactly one file at its declared `output_path` ([FT-166](FT-166)) has the same surface as a test-writing cell that may need to read several upstream outputs. The narrowest-authority shape the cell model implies is not expressible.

The pipeline factory demonstrates the per-step version of this discipline: `pipeline.yaml` declares `mcp_servers: [...]` per step and the step token's `allowed_servers` claim enforces it server-side — the verify step structurally cannot call `code-writer`. ADR-070 already borrowed the principle for roles and rejected the JWT machinery as multi-process overhead; what remains unadopted is the *granularity*: the factory scopes at its dispatch unit (the step), while we scope at the role and ignore our finer dispatch unit (the cell).

This ADR extends ADR-070 downward; it does not revisit it. The role surface remains the outer bound and the catalog remains the source of truth. It also inherits ADR-070's explicit deferral: sub-resource scoping (e.g. "may write only its declared `output_path`") stays deferred — cell narrowing selects *which tools*, not which arguments.

## Decision

**The effective tool surface of an LLM-backed cluster cell is `role surface ∩ cell narrowing`. The role surface comes from the role catalog at dispatch time; the cell may optionally declare a narrowing subset; nothing may widen beyond the role.**

1. **Cells gain an optional `tools` declaration.** A Cell ([FT-150](FT-150) substrate; `task_type` cell catalog today) may declare `tools: [..]` — a set of tool names. Absent declaration means "the full role surface" (today's effective behaviour, minus the hardcode).
2. **Dispatch resolves, intersects, and threads.** `cluster_dispatch` resolves the dispatching role's `dec:roleTool` set via the same `role_catalog::lookup()` the standard path uses, intersects it with the cell's declared `tools` (when present), and writes the result into `DispatchPayload.allowed_tools`. The `cluster_dispatch.rs:760` hardcode is removed.
3. **Widening is a declaration error.** A cell declaring a tool absent from the role surface fails the cluster run at validation time — before any cell dispatches — with a diagnostic naming the cell, the tool, and the role surface. It is not silently clamped: a clamp would hide a contract mismatch between the TaskType author's intent and the catalog.
4. **Empty intersection fail-closes loudly.** An empty effective surface is a dispatch-time error (same validation pass), mirroring the worker's `invalid_dispatch` fail-closed semantics from [ADR-070](ADR-070) §4 — but caught before the worker subprocess spawns, where it is cheap.
5. **Worker contract unchanged.** The worker keeps enforcing `allowed_tools` exactly as ADR-070 specifies. This ADR only fixes who computes the value on the cluster path and adds the cell term to the computation.
6. **Mechanical cells are out of band.** Cells without a model binding never dispatch a worker and carry no tool surface.

## Rationale

- **Restores the catalog as the single source of truth on the cluster path.** Tool-surface policy becomes one graph edit again, effective for both broad dispatches and cells; the audit trail (`dec session show` joining role → surface) becomes truthful for cluster sessions.
- **Expresses the narrowing the cell model already implies.** Decomposition under ADR-080 exists to shrink each decision's blast radius; the tool surface is the bluntest instrument of blast radius and was the one thing the cell could not narrow.
- **Fail-loud over fail-soft.** Both error modes (widening attempt, empty intersection) surface at validation, before tokens are spent — consistent with the cluster path's existing "validate the whole TaskType before dispatching cell one" posture ([FT-170](FT-170)).
- **Granularity matches the factory evidence.** The factory's per-step scoping is the proven version of this discipline at the dispatch-unit level; our dispatch unit is the cell.

## Rejected alternatives

- **Keep the hardcode, document it.** Leaves the catalog lying about cluster dispatches and per-cell narrowing inexpressible. The drift defect compounds as roles diversify (reviewer/judge cells with read-only intent).
- **Per-cell roles instead of cell narrowing.** Minting a `dec:Role` per cell kind would reuse ADR-070 unchanged, but explodes the role catalog (every TaskType × cell kind), and the role is the wrong identity — cells of different TaskTypes legitimately share the implementer role while needing different surfaces.
- **Cell declarations may widen with justification.** Widening breaks the invariant that the role catalog is the outer bound an operator can reason about from the graph alone; a cell needing more than its role grants is evidence the role binding is wrong, and that conversation belongs in the catalog, not in a TaskType override.
- **Silent clamp on widening declarations.** Cheaper than failing, but hides TaskType-author intent drifting from the catalog; the mismatch would surface later as a confusing cell failure ("tool not granted") instead of a clear validation error naming both sides.

## Test coverage

- A cluster cell's `DispatchPayload.allowed_tools` equals the role-catalog surface when the cell declares nothing (hardcode gone).
- A cell declaring a subset receives exactly the intersection.
- A cell declaring a tool outside the role surface fails the run at validation with a diagnostic naming cell, tool, and role surface — no worker spawns.
- An empty effective surface fail-closes at validation; the session record carries the structured failure.
