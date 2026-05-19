---
id: FT-030
title: 'decision-cli: Role catalog and authority declarations'
phase: 2
status: planned
depends-on:
- FT-019
adrs:
- ADR-013
- ADR-027
tests:
- TC-041
domains: []
domains-acknowledged:
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-030 produces no feedback artifacts.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-030's code is reorganised under that migration, not by this feature.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-030 has no feedback to gate.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-030 neither emits nor routes feedback.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-030 does not author or modify a fitness-function artifact.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-030 does not cross or alter that boundary.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-030 produces no feedback artifacts.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-030 neither emits nor consumes verdicts.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-030 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-030 produces no new Session or event type and inherits lineage from the harness.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-030 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-030 produces no action/interpretation pair.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-030 is out of scope for the pairing.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-030 runs after the working directory is resolved and does not re-discover it.
---

## Description

Generalise the role catalog introduced by [FT-019](FT-019) (verifier role only) into a first-class graph-resident catalog that holds every role with its authority declaration per [ADR-027](ADR-027). Migrate the slice-1 implementer role into the catalog. Add the implementer authority declaration. Provide the SHACL shape, the read API, the worker bundle injection of authority data.

This is the structural feature that makes slice 3's feedback flow coherent: workers can only emit `mustEscalate`-driven feedback if their bundle tells them which categories require escalation.

## Functional Specification

### Inputs

- The verifier role catalog entry from [FT-019](FT-019).
- The slice-1 implementer's hardcoded configuration.
- The bundle assembly path used by [FT-022](FT-022) (verifier dispatch) and the existing implementer dispatch from [FT-011](FT-011).

### Outputs

- Ontology extensions:
  - `dec:Role` (already present from [FT-019](FT-019), revalidated here)
  - `dec:Authority` class
  - `dec:mayDecide`, `dec:mustEscalate`, `dec:escalateVia`, `dec:rationale` properties on `dec:Authority`
  - `dec:authority` predicate from `dec:Role` to `dec:Authority`
- SHACL shape `dec:RoleShape` requiring every `dec:Role` to have exactly one `dec:authority` linking to a `dec:Authority` whose `mayDecide` and `mustEscalate` lists are non-empty.
- Seed authority declarations (Turtle under `core/role_catalog/seeds/`):
  - `implementer-authority.ttl` — mirrors the ADR-027 example.
  - `verifier-authority.ttl` — mayDecide: `verdict-classification`, `rationale-content`, `cited-references`. mustEscalate: `feature-spec-changes`, `adr-changes`, `cross-cutting-policy`.
- Rust struct `core::role_catalog::Authority`:
  ```rust
  pub struct Authority {
      pub may_decide: Vec<String>,
      pub must_escalate: Vec<String>,
      pub escalate_via: Vec<EscalationHint>,
      pub rationale: String,
  }
  pub struct EscalationHint {
      pub category: String,
      pub class: FeedbackClass,
      pub target_role: String,
  }
  ```
- Extended `core::role_catalog::Role` struct includes an `authority: Authority` field.
- Bundle assembly: the dispatch event payload for any role now includes the role's authority declaration as a structured field. Worker SDKs ([FT-031](FT-031)) expose it.
- Read API: `core::role_catalog::list_roles() -> Vec<Role>` (returns implementer + verifier in Phase A).

### State

- Two new `dec:Authority` artifacts in the orchestration store (one per Phase A role).
- The implementer role's previously-hardcoded configuration becomes a graph artifact.

### Behaviour

1. Extend the ontology with the `dec:Authority` class and predicates.
2. Author the SHACL shape `dec:RoleShape` and `dec:AuthorityShape`.
3. Author the two authority seeds.
4. Extend init to seed both roles' authorities; provide a migration for pre-existing slice-1 stores.
5. Extend `core::role_catalog::Role` to carry the authority. Refactor [FT-019](FT-019)'s lookup function to populate it.
6. Update bundle assembly:
   - `core::bundle::assemble_for_role(role_id, …)` reads the role's authority and includes it in the payload.
   - Worker SDK exposes `bundle.authority: Authority` (per [FT-031](FT-031)).
7. Per slice-level SDP: this module is `core::role_catalog`. Slice-2 and slice-3 features that need authority data import from here.

### Invariants

- Every `dec:Role` in the store has exactly one `dec:Authority`.
- Every `dec:Authority`'s `mayDecide` and `mustEscalate` lists are non-empty and disjoint (SHACL `sh:sparql` constraint).
- Every category referenced in `mustEscalate` has at least one `dec:escalateVia` entry naming a feedback class and target role.
- The implementer's `mustEscalate` includes at least `feature-spec-changes`, `adr-changes`, `cross-cutting-policy` (the slice-3 baseline).
- A dispatched worker bundle always includes the role's authority section.

### Error handling

- Missing authority on a role → SHACL violation, refused at write/init time.
- Authority with overlapping `mayDecide` / `mustEscalate` → SHACL `sh:sparql` violation.
- Bundle assembly with no role binding (e.g. unknown role id) → `BundleError::UnknownRole { id }`.

### Boundaries

- **In scope.** Authority schema, seeds, Rust types, catalog generalisation, bundle injection, implementer migration.
- **Out of scope.** Workers consuming `bundle.authority` ([FT-031](FT-031)). Enforcement that workers emit feedback for `mustEscalate` categories (a slice-3 TC asserts this from session telemetry — lives in the test layer).

## Out of scope

- Dynamic authority updates (Phase B+).
- Per-feature authority overrides (Phase C+ — authority is role-stable by design).
- Authority composition / inheritance (Phase C+).
