---
id: FT-019
title: 'decision-cli: Verifier role catalog entry'
phase: 2
status: complete
depends-on:
- FT-006
- FT-009
adrs:
- ADR-017
- ADR-019
- ADR-027
tests:
- TC-027
- TC-088
domains: []
domains-acknowledged:
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-019 runs after the working directory is resolved and does not re-discover it.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-019 neither emits nor routes feedback.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-019 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-019 does not author or modify a fitness-function artifact.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-019 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-019 produces no action/interpretation pair.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-019 has no feedback to gate.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-019 produces no feedback artifacts.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-019 produces no feedback artifacts.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-019 neither emits nor consumes verdicts.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-019's code is reorganised under that migration, not by this feature.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-019 produces no new Session or event type and inherits lineage from the harness.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-019 does not cross or alter that boundary.
---

## Description

Land the verifier role as a graph artifact in the orchestration store's role catalog. Slice 1 ([FT-011](FT-011), [FT-013](FT-013)) wired a single hardcoded "implementer" role inline. Phase A introduces a second role, so the catalog moves from hardcoded constants in `core/` to a queryable graph artifact pattern that supports per-role bundles, per-role model bindings (still hardcoded for Phase A but per-role), and per-role authority declarations ([ADR-027](ADR-027) — landed by [FT-030](FT-030)).

This feature lands the *verifier* entry specifically. Generalising the catalog and migrating implementer into it is part of [FT-030](FT-030) (role catalog + authority).

## Functional Specification

### Inputs

- The orchestration store (post-`dec init`).
- The base ontology ([FT-006](FT-006)) extended with `dec:Role` type, `dec:Authority` type, and the relevant predicates (`dec:roleId`, `dec:roleInputType`, `dec:roleOutputType`, `dec:roleModelBinding`).
- The hardcoded implementer role configuration in `core/` (slice 1).

### Outputs

- A `dec:Role` artifact in the orchestration store with `dec:roleId = "verifier"`, input type `dec:CodeChange + dec:FeatureSpec + dec:BundleHash`, output type `dec:VerificationVerdict` (see [FT-020](FT-020)).
- A `core/role_catalog/` module that exposes a `Role` struct and a registry function returning the catalog as a `Vec<Role>` derived from SPARQL.
- A small Rust API: `core::role_catalog::lookup(role_id: &str) -> Option<Role>` used by `features/ft_021_dispatch_group/` to dispatch the verifier.

### State

- New artifact in orchestration store: `<role:verifier> a dec:Role ; …`. Seeded at `dec init` time after slice 2 ships (existing init runs gain the seed via the bootstrap subscription pattern; new init runs include the verifier from the start).
- No removal of state. Implementer's hardcoded entry stays until [FT-030](FT-030) migrates it into the catalog.

### Behaviour

1. Extend the embedded ontology ([FT-006](FT-006)) with the `dec:Role` and `dec:Authority` classes and their SHACL shapes.
2. Author a Turtle seed (`crates/decision-cli/src/core/role_catalog/seeds/verifier.ttl`) containing the verifier's catalog entry.
3. Extend `dec init` to load this seed into the orchestration store as part of bootstrap (alongside the bundled ValueAction definitions per [FT-007](FT-007)). For pre-existing stores: the next `dec init --reseed` (or one-off migration script under `scripts/migrations/`) injects the seed.
4. Expose `core::role_catalog::Role` and `core::role_catalog::lookup`. These read SPARQL against the orchestration store, with no caching (slice 2 scale — N=2 roles).
5. Per the slice-level SDP convention in `CLAUDE.md`, no `features/*` directory imports `features/ft_021_dispatch_group/` or any sibling — the verifier role is read through `core::role_catalog`.

### Invariants

- The store always contains exactly one `dec:Role` with `dec:roleId = "verifier"` after init.
- The verifier role's output type matches [FT-020](FT-020)'s `dec:VerificationVerdict` IRI.
- `core::role_catalog::lookup("verifier")` is total (returns `Some(Role)`) post-init.

### Error handling

- Missing seed file at init time → `InitError::MissingRoleSeed { role_id, path }` (consistent with existing `InitError` shape).
- Malformed seed Turtle → SHACL violation, same path as the ValueStream SHACL violations per [ADR-006](ADR-006).
- Lookup against an uninitialised store → return `None`; callers (e.g. [FT-021](FT-021)) raise a structured error.

### Boundaries

- **In scope.** Schema extension for `dec:Role`, the verifier seed, the read API in `core::role_catalog`, init wiring.
- **Out of scope.** Generalising the implementer role into the catalog ([FT-030](FT-030)). Authority declarations ([FT-030](FT-030)). Per-role model selection ([ADR-020](ADR-020) keeps Phase A model bindings hardcoded). Catalog editing CLI surface (later slice).

## Out of scope

- Dynamic role registration at runtime (Phase B at earliest).
- Per-role policy artifacts (Phase B).
- Model catalog as graph artifact (deferred per slice-1 bounds §6.2).
