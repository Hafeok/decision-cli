---
id: FT-056
title: 'decision-cli: Bundle.stakes field and enum SHACL constraint'
phase: 2
status: complete
depends-on: []
adrs:
- ADR-008
- ADR-015
- ADR-017
- ADR-018
- ADR-020
- ADR-021
- ADR-022
- ADR-023
- ADR-024
- ADR-025
- ADR-027
- ADR-033
- ADR-034
- ADR-035
- ADR-036
- ADR-037
tests:
- TC-102
domains:
- data-model
domains-acknowledged: {}
---

## Description

Extend the `dec:Bundle` artifact type with a `dec:stakes` datatype property per [ADR-035](ADR-035). The field is a closed enum of three values (`routine`, `elevated`, `foundational`), required on every bundle, default `routine` at composition time. This is the field the escalation policy from [ADR-034](ADR-034) reads via the `stakes_routine` / `stakes_elevated` / `stakes_foundational` trigger signals, and the field the `reasoning_effort` mapping from [FT-063](FT-063) reads when the resolved capability has `configurable_effort = true`.

The bundle composer (`core::bundle::assemble_for_role`) sets stakes via the default ladder from [ADR-035](ADR-035) §"Who sets it"; this feature wires the field into the ontology, the SHACL shape, the composer's default logic, and the bundle Rust struct.

## Functional Specification

### Inputs

- The embedded base ontology ([FT-006](FT-006)) — `dec:Bundle` class already exists.
- `core::bundle::Bundle` Rust struct exists in the bundle module.
- The bundle composer `core::bundle::assemble_for_role` already runs per dispatch and reads the focal artifact.

### Outputs

- New ontology term:
  - `dec:stakes` (xsd:string, required on `dec:Bundle`) — enum: `routine`, `elevated`, `foundational`.
- Extended SHACL shape `dec:BundleShape`:
  - `sh:property` for `dec:stakes` with `sh:datatype xsd:string`, `sh:minCount 1`, `sh:maxCount 1`, `sh:in ("routine" "elevated" "foundational")`.
- Extended Rust type:
  ```rust
  pub enum Stakes { Routine, Elevated, Foundational }
  impl Stakes { pub fn as_str(&self) -> &'static str; pub fn try_from_str(s: &str) -> Result<Self, …>; }
  pub struct Bundle {
      // … existing fields …
      pub stakes: Stakes,
  }
  ```
- Default ladder logic in `core::bundle::default_stakes_for(focal_artifact) -> Stakes`:
  - Focal is `dec:Capability`, `dec:RoleBinding`, an ontology change, or a new artifact type definition → `Foundational`.
  - Focal is a cross-cutting ADR (per [ADR-014](ADR-014)) or a feature_spec linked to ≥ 2 cross-cutting ADRs → `Elevated`.
  - Otherwise → `Routine`.

### State

- Embedded ontology + shapes bytes grow by ~10 lines.
- All bundle composition paths now set `bundle.stakes` explicitly (no `Option<Stakes>`; `Default::default()` returns `Routine`).
- Existing in-flight sessions: bundles already in the graph without `dec:stakes` are migrated to `Routine` by the bootstrap migration step in [FT-058](FT-058).

### Behaviour

1. Extend the ontology Turtle with the `dec:stakes` predicate.
2. Extend the shapes Turtle with the `sh:in` enum constraint on the `dec:Bundle` shape.
3. Add the `Stakes` enum + `as_str` / `try_from_str` impls; wire it into `core::bundle::Bundle`.
4. Implement `default_stakes_for` per the ladder above. Reads the focal artifact's class IRI from the bundle's `dec:focal` link and the linked ADRs' `dec:scope` field (see [ADR-014](ADR-014)).
5. `core::bundle::assemble_for_role` calls `default_stakes_for` and sets `bundle.stakes` before SHACL validation runs. The composer may *override* the default before write (the meta-loop sets stakes deliberately when proposing catalog edits); the override mechanism is a constructor parameter `with_stakes(Stakes)` on the bundle builder.
6. Bundle serialisation (the markdown form workers receive) includes a `## Dispatch metadata` section listing `Stakes: <value>` so workers and humans reading session logs can see it. This is informational; nothing in the worker contract is contingent on stakes.

### Invariants

- Every `dec:Bundle` in the graph has exactly one `dec:stakes` literal.
- The literal is one of the three enum values (SHACL refuses everything else).
- The default ladder is deterministic — same focal artifact + same ADR scope set → same stakes value.
- `default_stakes_for` is pure: no graph reads beyond what the bundle composer already holds.

### Error handling

- SHACL violation on missing or invalid stakes → graph write refused.
- A bundle composer call with a focal artifact of an unrecognised class falls through to `Routine` (no error; conservative default).
- The migration step that backfills existing bundles is idempotent: re-running it on a graph where every bundle already has stakes is a no-op.

### Boundaries

- **In scope.** Ontology extension, SHACL extension, Rust enum + struct field, default ladder, composer integration, markdown serialisation, migration for existing bundles.
- **Out of scope.** Workers reading stakes for behavioral decisions (workers don't act on stakes — only the dispatcher does, via [FT-062](FT-062) and [FT-063](FT-063)).
- **Out of scope.** Per-feature stakes overrides (rejected by [ADR-035](ADR-035) — stakes is per-bundle).

## Out of scope

- Richer stakes ontology (more than three values). Adding values requires an ADR amendment + vocabulary extension.
- Inferring stakes from runtime cost / latency observations (Phase 3+ meta-loop work).
- A `dec stakes set <bundle> --stakes foundational` CLI override — bundles are immutable after composition; reauthor via supersession if needed.
