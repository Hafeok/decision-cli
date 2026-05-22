---
id: FT-058
title: 'decision-cli: Capability and RoleBinding catalog bootstrap from seed YAML'
phase: 2
status: planned
depends-on:
- FT-054
- FT-055
- FT-056
- FT-057
adrs:
- ADR-036
- ADR-037
- ADR-002
tests:
- TC-104
domains:
- data-model
- storage
domains-acknowledged: {}
---

## Description

Bootstrap the seed capability and role-binding catalog from a versioned YAML source per [ADR-036](ADR-036). The YAML lives in the repo at `crates/decision-cli/seeds/capabilities.yaml` and `seeds/role_bindings.yaml`. A bootstrap step in `dec init` reads the YAML and writes one `dec:Capability` (or `dec:RoleBinding`) artifact per entry into the orchestration store via `GraphWriter`. After bootstrap, the graph is authoritative; the YAML is documentation.

The seed *content* — which capabilities exist and which roles bind to what — is the policy described in [ADR-037](ADR-037). This feature lands the bootstrap mechanism and the initial seed values from the PRD's §5.2 and §6.2 tables.

## Functional Specification

### Inputs

- `crates/decision-cli/seeds/capabilities.yaml` — the catalog of `Capability` entries per PRD §5.2.
- `crates/decision-cli/seeds/role_bindings.yaml` — the active `RoleBinding` entries per PRD §6.2.
- The `dec:Capability` schema from [FT-054](FT-054).
- The `dec:RoleBinding` / `EscalationStep` / `EscalationTrigger` schema from [FT-055](FT-055).
- The `dec:Bundle.stakes` extension from [FT-056](FT-056) (so the migration step can backfill existing bundles).
- The `dec:SessionRecord` escalation edges from [FT-057](FT-057) (no backfill needed; the fields are optional).
- `GraphWriter` ([FT-001](FT-001)) for transactional writes.
- The init flow ([FT-008](FT-008), [FT-009](FT-009)) which runs the bootstrap pipeline.

### Outputs

- Seed YAML files committed under `crates/decision-cli/seeds/`. The PRD-defined initial content:
  - `capabilities.yaml` — 10 entries: `classifier`, `code-writer`, `code-writer-heavy`, `standard-reasoning`, `standard-reasoning-frontier`, `deep-reasoning`, `vision-gui`, `vision-general`, `embedding`, `audio-transcribe`. Schema mirrors [FT-054](FT-054)'s `dec:Capability`.
  - `role_bindings.yaml` — 5 entries: `implementer`, `verifier`, `architect`, `test_interpreter`, `feedback_class_triager`, each with `default_capability` and ordered `escalation_steps` from PRD §6.2.
- New Rust module `core::bootstrap::catalog` exposing:
  - `pub fn bootstrap_catalog(graph: &GraphWriter, capabilities_yaml: &Path, bindings_yaml: &Path) -> Result<BootstrapReport, BootstrapError>`.
  - `BootstrapReport { capabilities_written: Vec<CapabilityId>, bindings_written: Vec<String>, bundles_migrated: usize, source_hashes: (String, String) }`.
- Init flow extension: `dec init` runs `bootstrap_catalog` after the existing ontology / value-stream seeding, before the value-action seeding. Idempotent: if the active catalog already matches the YAML's source hash on `dec:bootstrap_source`, the bootstrap is a no-op.
- Migration step for existing bundles: `core::bootstrap::migrate_bundle_stakes(graph) -> Result<usize, …>` walks every `dec:Bundle` lacking `dec:stakes` and inserts `dec:stakes "routine"` per [FT-056](FT-056)'s default.

### YAML schema (informational)

```yaml
# capabilities.yaml
capabilities:
  - capability_id: code-writer
    endpoint: scaleway
    model_identifier: qwen3-coder-30b-a3b-instruct
    tier: 1
    context_window: 128000
    max_output: 32000
    supports_vision: false
    supports_tool_calling: true
    cost_input_per_m: 0.20
    cost_output_per_m: 0.80
    configurable_effort: false
    status: active
    version: 1
    notes: ""

# role_bindings.yaml
bindings:
  - role_id: implementer
    default_capability: code-writer
    escalation_steps:
      - capability: code-writer-heavy
        triggers: [prior_attempts_ge_3, audit_fail, feedback_unimplementable_critical]
      - capability: deep-reasoning
        triggers: [stakes_foundational, prior_attempts_ge_5]
    version: 1
    active: true
```

### State

- 10 `Capability` artifacts in the orchestration store after first bootstrap.
- 5 `RoleBinding` artifacts in the orchestration store after first bootstrap.
- Existing bundles backfilled with `stakes = routine`.
- The YAML content hash is recorded on every artifact's `dec:bootstrap_source` field; subsequent bootstraps compare the hash and skip if unchanged.

### Behaviour

1. Load both YAML files. Parse via `serde_yaml` into typed Rust structs that mirror [FT-054](FT-054)'s `Capability` and [FT-055](FT-055)'s `RoleBinding`.
2. Compute SHA-256 of each YAML file (canonicalised by stripping trailing whitespace, normalising line endings).
3. For each capability entry: construct Turtle, write via `GraphWriter`. SHACL validation runs; failure rolls back the bootstrap with a specific YAML line-number error.
4. For each role binding entry: construct Turtle (including the `rdf:List` of `EscalationStep` and `rdf:Bag` of `EscalationTrigger`); write via `GraphWriter`. SHACL validation runs.
5. Run `migrate_bundle_stakes` to backfill `dec:stakes = routine` on existing bundles. Idempotent: bundles already carrying stakes are skipped.
6. Return `BootstrapReport` for the init log.
7. `dec init`'s human-readable summary includes "catalog: 10 capabilities, 5 role bindings (active)" so operators see the catalog was seeded.

### Invariants

- The bootstrap is atomic per pipeline (catalogue + binding writes in one transaction; failure rolls back both). `GraphWriter` provides this.
- The seed YAML content matches the PRD's §5.2 and §6.2 tables byte-for-byte (a TC asserts this so the PRD and the seed cannot drift unnoticed).
- Re-running `dec init` on an already-bootstrapped store with unchanged YAML is a no-op (idempotent).
- A catalog change between two `dec init` runs (YAML edit) results in *new versioned artifacts* with `dec:supersedes` links to the prior versions, not in-place mutation. The graph keeps full history.
- Bundle migration runs exactly once per bundle (idempotent on `dec:stakes` presence).

### Error handling

- YAML parse error → `BootstrapError::YamlParseFailed { file, line, column }`; init aborts.
- SHACL violation on a constructed artifact → `BootstrapError::ShaclViolated { artifact_id, report }`; init aborts; the entire bootstrap is rolled back (no partial catalog).
- Unknown trigger signal in YAML → caught at SHACL time via [FT-055](FT-055)'s `sh:in` constraint; same path as above.
- Reference to a non-existent capability from a binding (`default_capability: code-writer` when no such capability was loaded) → `BootstrapError::UnresolvedReference { binding, missing_capability }`; the YAML loader checks ordering by loading capabilities first.

### Boundaries

- **In scope.** YAML files, parser, Turtle construction, `bootstrap_catalog`, `migrate_bundle_stakes`, init integration, idempotency.
- **Out of scope.** Reading the YAML at dispatch time (the graph is authoritative — [ADR-036](ADR-036)).
- **Out of scope.** A `dec capability sync` command to regenerate YAML from the graph (mentioned in [ADR-036](ADR-036) as future work; not in this feature).
- **Out of scope.** Editing the catalog via CLI (the catalog is editable via `GraphWriter`; a friendly `dec capability set` is a later feature_spec).

## Out of scope

- Hot-reloading the catalog without restarting `dec` (the init step is the only loader; long-running daemons would need a separate feature_spec for hot reload).
- Multi-source seeds (e.g. per-environment YAML overlays). The bootstrap reads one file each; environment-specific deployment is operator concern.
- A migration that auto-supersedes the prior catalog when the YAML changes; today's behavior is "new versioned artifacts written, prior versions remain active until explicitly superseded by a graph write". A future `dec catalog promote` feature_spec can add this.
