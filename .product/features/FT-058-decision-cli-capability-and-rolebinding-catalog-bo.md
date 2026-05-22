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

Bootstrap the seed capability and role-binding catalog from a versioned YAML source per [ADR-036](ADR-036) and PRD §10.7. The YAML lives in the repo at `config/capabilities.yaml` and `config/role-bindings.yaml`; the populator script lives at `scripts/bootstrap_catalog.py`. The script reads both YAML files and writes one `dec:Capability` (or `dec:RoleBinding`) artifact per entry into the orchestration store via `GraphWriter`. After bootstrap, the graph is authoritative; the YAML is documentation.

The seed *content* — which capabilities exist and which roles bind to what — is the policy described in [ADR-037](ADR-037). This feature lands the bootstrap mechanism with **strict divergence handling** (the script errors and exits on YAML/graph mismatch; it never silently overwrites graph state) and the initial seed values from the PRD's §5.2 (12 entries) and §6.2 (5 bindings) tables.

## Functional Specification

### Inputs

- `config/capabilities.yaml` — the catalog of `Capability` entries per PRD §5.2 (12 entries: `classifier`, `code-writer`, `code-writer-heavy`, `standard-reasoning`, `standard-reasoning-frontier`, `deep-reasoning`, `mid-reasoning`, `fast-reasoning`, `vision-gui`, `vision-general`, `embedding`, `audio-transcribe`).
- `config/role-bindings.yaml` — the active `RoleBinding` entries per PRD §6.2 (5 entries: `implementer`, `verifier`, `architect`, `test_interpreter`, `feedback_class_triager`).
- The `dec:Capability` schema from [FT-054](FT-054), including the cost-currency, cache-rate, and reasoning-trace fields.
- The `dec:RoleBinding` / `EscalationStep` / `EscalationTrigger` schema from [FT-055](FT-055).
- The `dec:Bundle.stakes` extension from [FT-056](FT-056) (so the migration step can backfill existing bundles).
- The `dec:SessionRecord` extensions from [FT-057](FT-057) (token-breakdown fields; existing Anthropic sessions get backfilled with `input_tokens_base = <prior input_tokens>` and zero cache fields).
- `GraphWriter` ([FT-001](FT-001)) for transactional writes.
- The init flow ([FT-008](FT-008), [FT-009](FT-009)) which can shell out to `scripts/bootstrap_catalog.py` or invoke its `bootstrap_catalog()` entry point directly.

### Outputs

- Seed YAML files committed under `config/` (repo-root-relative — paths fixed by [ADR-036](ADR-036)):
  - `config/capabilities.yaml` — 12 entries. Schema mirrors [FT-054](FT-054)'s `dec:Capability` including `cost_currency`, optional `cost_cache_hit_per_m` / `cost_cache_write_5m` on Anthropic entries, and `exposes_reasoning_trace` on `standard-reasoning-frontier`.
  - `config/role-bindings.yaml` — 5 entries with default_capability and ordered escalation_steps from PRD §6.2.
- Bootstrap script `scripts/bootstrap_catalog.py` (Python, executable, accepts `--graph-path <path>` and `--dry-run`):
  - Loads both YAML files (capabilities first, then bindings).
  - Validates ordering: any binding referencing a capability not in the loaded capabilities errors and exits before any write.
  - For each YAML entry: computes the stable identifier `(capability_id, version)` or `(role_id, version)`, queries the graph, and applies the three-way decision in "Idempotence" below.
  - All writes from a single bootstrap run happen in a single `GraphWriter` transaction; SHACL violations or divergence errors roll back the entire run.
- Optional Rust convenience wrapper `core::bootstrap::catalog::bootstrap_catalog(graph, capabilities_yaml, bindings_yaml) -> Result<BootstrapReport, BootstrapError>` for callers that prefer in-process invocation; the Python script is the authoritative entry point per PRD §10.7.
- Migration step for existing artifacts:
  - `core::bootstrap::migrate_bundle_stakes(graph) -> Result<usize, …>` walks every `dec:Bundle` lacking `dec:stakes` and inserts `dec:stakes "routine"` per [FT-056](FT-056)'s default.
  - `core::bootstrap::migrate_session_token_breakdown(graph) -> Result<usize, …>` walks every `dec:SessionRecord` lacking the three new token-breakdown fields and inserts `input_tokens_base = <existing input_tokens triple, if any, else 0>`, `input_tokens_cache_write = 0`, `input_tokens_cache_hit = 0`.

### YAML schema (informational)

```yaml
# config/capabilities.yaml
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
    cost_currency: EUR
    configurable_effort: false
    exposes_reasoning_trace: false
    status: active
    version: 1
  - capability_id: deep-reasoning
    endpoint: anthropic
    model_identifier: claude-opus-4-7
    tier: 3
    context_window: 200000
    max_output: 32000
    supports_vision: true
    supports_tool_calling: true
    cost_input_per_m: 5.00
    cost_output_per_m: 25.00
    cost_cache_hit_per_m: 0.50
    cost_cache_write_5m: 6.25   # 5m write rate; verify against Anthropic pricing at bootstrap time
    cost_currency: USD
    configurable_effort: false
    exposes_reasoning_trace: false
    status: active
    version: 1
  - capability_id: standard-reasoning-frontier
    endpoint: scaleway
    model_identifier: qwen3.5-397b-a17b
    tier: 2
    context_window: 250000
    max_output: 16000
    supports_vision: true
    supports_tool_calling: true
    cost_input_per_m: 0.60
    cost_output_per_m: 3.60
    cost_currency: EUR
    configurable_effort: false
    exposes_reasoning_trace: true     # qwen3.5-397b emits message.reasoning per PRD §10.6
    status: active
    version: 1
  # ... (remaining 9 entries from §5.2 including mid-reasoning + fast-reasoning candidates)

# config/role-bindings.yaml
role_bindings:
  - role_id: implementer
    default_capability: code-writer
    escalation_steps:
      - capability: code-writer-heavy
        triggers: [prior_attempts_ge_3, audit_fail, feedback_unimplementable_critical]
      - capability: deep-reasoning
        triggers: [stakes_foundational, prior_attempts_ge_5]
    version: 1
    active: true
  # ... (remaining 4 entries from §6.2)
```

### State

- 12 `Capability` artifacts in the orchestration store after first bootstrap (10 active + 1 preview + 1 with status `preview` and 2 `candidate`-status — see PRD §5.2 status column).
- 5 `RoleBinding` artifacts in the orchestration store after first bootstrap.
- Existing bundles backfilled with `stakes = routine`.
- Existing Anthropic sessions backfilled with the three token-breakdown fields (base = original count, cache fields = 0).
- The YAML content hash is recorded on every artifact's `dec:bootstrap_source` field; subsequent bootstraps compare against this for the divergence decision below.

### Behaviour

1. **Load.** Read both YAML files. Parse via `pyyaml` (Python script) into typed dataclasses that mirror [FT-054](FT-054)'s `Capability` and [FT-055](FT-055)'s `RoleBinding`. Compute SHA-256 of each YAML file (canonicalised by stripping trailing whitespace, normalising line endings).
2. **Ordering check.** For each binding entry, assert that `default_capability` and every `step_capability` references a capability_id present in the loaded capabilities. If not, error with `BootstrapError::UnresolvedReference { binding, missing_capability }` before any graph write.
3. **Bootstrap capabilities first.** For each capability entry:
   - Compute stable identifier `(capability_id, version)`.
   - Query the graph: does an artifact with that identifier exist?
   - **If yes and content matches** (compare every property field against graph values, ignoring `dec:bootstrap_source`): skip silently.
   - **If yes and content differs** (any property field disagrees): error with `BootstrapError::Divergence { artifact_id, yaml_value, graph_value, hint: "the graph is authoritative; update the YAML to match, or bump the YAML version field and re-run to create a new versioned artifact" }`. Exit non-zero. **No writes happen.** The transaction rolls back.
   - **If no**: construct Turtle, write via `GraphWriter`. SHACL validation runs; failure rolls back the entire bootstrap with a specific YAML line-number error.
4. **Bootstrap bindings second.** Same three-way decision for each role-binding entry.
5. **Migrate bundles and sessions** (only on first bootstrap or when explicitly opted in via `--migrate`):
   - `migrate_bundle_stakes` backfills `dec:stakes = routine` on existing bundles. Idempotent.
   - `migrate_session_token_breakdown` backfills the three new fields on existing Anthropic sessions. Idempotent.
6. **Report.** Return / print `BootstrapReport { capabilities_written, capabilities_skipped, capabilities_divergent, bindings_*, bundles_migrated, sessions_migrated, source_hashes }`. Init log surfaces the summary: `"catalog: 12 capabilities (12 new), 5 role bindings (5 new), 0 divergent"` on first run; `"catalog: unchanged"` on idempotent re-run.

### Invariants

- The bootstrap is atomic per run: capability writes + binding writes + migration writes in one `GraphWriter` transaction; any failure rolls them all back. `GraphWriter` provides this.
- The seed YAML content matches the PRD's §5.2 and §6.2 tables byte-for-byte (a TC asserts this so the PRD and the seed cannot drift unnoticed).
- Re-running `scripts/bootstrap_catalog.py` against an already-bootstrapped store with unchanged YAML is a no-op (idempotent — every artifact matches, every check skips).
- Re-running against a graph state that diverges from YAML errors out with a human-readable diff per artifact. **Bootstrap never silently overwrites graph state.**
- A YAML edit that changes content without bumping `version` causes divergence error on re-run — the operator must decide: revert the YAML, or bump the version to create a new versioned artifact alongside.
- Bundle and session migrations run exactly once per artifact (idempotent on field presence).

### Error handling

- YAML parse error → `BootstrapError::YamlParseFailed { file, line, column }`; exit non-zero.
- SHACL violation on a constructed artifact → `BootstrapError::ShaclViolated { artifact_id, report }`; exit non-zero; the entire bootstrap rolls back (no partial catalog).
- Unknown trigger signal in YAML → caught at SHACL time via [FT-055](FT-055)'s `sh:in` constraint; same path as above.
- Reference to a non-existent capability from a binding (caught at ordering check) → `BootstrapError::UnresolvedReference { binding, missing_capability }`; exit non-zero before any write.
- Divergence between YAML and existing graph artifact → `BootstrapError::Divergence { ... }` with a per-field diff; exit non-zero. **This is the load-bearing rule of [ADR-036](ADR-036); the script must refuse to silently update graph state.**

### Boundaries

- **In scope.** YAML files at `config/`, Python script at `scripts/bootstrap_catalog.py`, Turtle construction, divergence handling, ordering enforcement, migration helpers, init integration, idempotency.
- **Out of scope.** Reading the YAML at dispatch time (the graph is authoritative — [ADR-036](ADR-036)).
- **Out of scope.** A `dec capability sync` command to regenerate YAML from the graph (mentioned in [ADR-036](ADR-036) as future work; not in this feature).
- **Out of scope.** Editing the catalog via CLI (the catalog is editable via `GraphWriter`; a friendly `dec capability set` is a later feature_spec).
- **Out of scope.** Bumping versions in the YAML automatically when divergence is detected (the operator decides; the script errors out).

## Out of scope

- Hot-reloading the catalog without restarting `dec` (the init step is the only loader; long-running daemons would need a separate feature_spec for hot reload).
- Multi-source seeds (e.g. per-environment YAML overlays). The bootstrap reads one file each; environment-specific deployment is operator concern.
- A migration that auto-supersedes the prior catalog when the YAML changes; today's behavior is "new versioned artifacts written, prior versions remain active until explicitly superseded by a graph write". A future `dec catalog promote` feature_spec can add this.
- Anthropic 1-hour cache TTL pricing variants (out of catalog scope per PRD §5.2).
