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
domains-acknowledged: {}
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
