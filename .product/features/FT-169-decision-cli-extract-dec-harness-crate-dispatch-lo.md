---
id: FT-169
title: 'decision-cli: Extract dec-harness crate — dispatch loop, cluster dispatch, drive planners, and worker contract'
phase: 5
status: planned
depends-on:
- FT-168
adrs:
- ADR-086
- ADR-072
tests:
- TC-411
- TC-415
domains: []
domains-acknowledged:
  ADR-083: ADR-083 governs tech-detail binding levels for archetypes. FT-169 is a pure relocation of harness machinery; no tech detail is bound at any level.
  ADR-084: ADR-084 mandates seam audits for archetypes. FT-169 ships no archetype; the three-scope audit pipeline (FT-153) lands later in dec-harness.
  ADR-082: ADR-082 governs archetype-layer semantics. FT-169 relocates orchestration machinery; archetype-dispatch behaviour (FT-153, FT-157) lands later in the crate this slice creates.
  ADR-081: ADR-081 governs CLI enumerate/lookup verb pairs. FT-169 relocates dispatch/drive/worker machinery into crates/dec-harness adding no CLI verbs; clap trees stay in decision-cli unchanged.
---

## Description

Third and final migration slice for [ADR-086](ADR-086). Extracts the orchestration machinery out of `crates/decision-cli/src/core/` into a new workspace crate `crates/dec-harness/`: the dispatch loop and dispatch sessions, cluster dispatch ([ADR-080](ADR-080)), drive planners (ship, def-ready, readiness orchestrator), worker contract and worker resolution, subscriptions, role catalog, bundle assembly orchestration, and capability resolution/escalation ([ADR-033](ADR-033), [ADR-034](ADR-034)).

After this slice, `decision-cli` is what [ADR-016](ADR-016) always said it should be — the volatile outer ring: clap trees, `main.rs` wiring, the MCP shim, and vertical feature slices that compose the three stable crates beneath them.

## Functional Specification

### Inputs

- `crates/decision-cli/src/core/` modules: `dispatch/`, `dispatch_session.rs`, `drive/`, `worker/`, `worker_curator/`, `worker_manifest/`, `subscriptions/`, `role_catalog/`, `task_type/`, `bootstrap/`, `handler.rs`, plus the orchestration halves left behind by [FT-168](FT-168) splits, and supporting modules (`identity_verifier/`, `cosign_trust/`, `oci_manifest/`, `sbom_referrer/`, `metrics/`, `verify/`) as their dependency direction dictates.
- `crates/dec-graph/` and `crates/dec-ontology/` from [FT-167](FT-167)/[FT-168](FT-168).
- `product-core` ([ADR-077](ADR-077)) — consumed here for product-graph reads during planning.

### Outputs

- `crates/dec-harness/` — new workspace member. `Cargo.toml` declares `dec-graph`, `dec-ontology`, `oxi-events`, `product-core`, runtime crates (`tokio`, `reqwest`, `serde`, `tracing`, `anyhow`/`thiserror` per convention). **Never** `clap`, never `decision-cli`.
- Facade re-exports in `crates/decision-cli/src/core/mod.rs` for the moved modules, preserving feature-slice import paths.
- `crates/decision-cli/src/core/` after this slice contains only facades and any genuinely CLI-coupled residue (which should trend to zero).

### State

- No graph-resident state changes. The dispatch protocol ([ADR-045](ADR-045), [ADR-046](ADR-046)), worker contract ([ADR-008](ADR-008)), and session recording ([ADR-050](ADR-050)) are behaviour-identical.

### Behaviour

1. Create the crate and move the listed modules, importing graph access from `dec-graph` and domain types from `dec-ontology`.
2. Modules whose placement is ambiguous (e.g. `verify/`, `metrics/`) are placed by the dependency rule — if it dispatches, plans, or talks to workers it lands here; if a feature slice merely formats its output, that formatting stays in the slice.
3. Add facades; workspace compiles; no feature-slice import churn.
4. `cargo test --workspace` green; `product verify --platform` green; the topology TC now asserts the full ADR-086 graph with all three crates present.

### Invariants

- `dec-harness` does not depend on `decision-cli` or `clap` (enforced by `scripts/checks/crate-dependency-topology.sh`).
- No workspace crate depends on `decision-cli`.
- Cluster dispatch, drive, and worker integration tests pass unmodified.

### Error handling

- No new runtime error surface; topology violations exit 1 via the topology TC.

### Boundaries

- The MCP shim and all clap argument types stay in `decision-cli` — the harness is invocable from any front end.
- [ADR-016](ADR-016)'s intra-crate rules continue to govern `decision-cli`'s feature slices unchanged.

## Out of scope

- Behaviour changes to dispatch, drive, escalation, or the worker protocol.
- Facade burn-down; per-feature crates (still rejected per ADR-016/ADR-086).
- Any archetype-layer functionality ([FT-147](FT-147)–[FT-160](FT-160)).