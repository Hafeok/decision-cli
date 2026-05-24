---
id: TC-104
title: Catalog bootstrap seeds 13 Capability and 6 RoleBinding artifacts idempotently
type: exit-criteria
status: passing
validates:
  features:
  - FT-058
  adrs:
  - ADR-036
phase: 2
runner: bash
runner-args: tests/scripts/tc-104-catalog-bootstrap.sh
runner-timeout: 180
last-run: 2026-05-24T19:14:23.673322616+00:00
last-run-duration: 1.1s
---

## Description

Scenario: `scripts/bootstrap_catalog.py` against an empty orchestration store seeds the catalog from `config/capabilities.yaml` and `config/role-bindings.yaml` per [FT-058](FT-058) / [ADR-036](ADR-036), producing exactly 13 `dec:Capability` and 6 `dec:RoleBinding` artifacts. A second run against the same store with unchanged YAML is a no-op. A run against a store with divergent state errors out without writing.

The runner is `bash` driving the Python bootstrap script against a temp store, asserting via `dec capability list` / `dec binding list` (or direct SPARQL via `oxigraph::sparql`) the expected counts and contents.

Acceptance:

1. **First-bootstrap counts.** After `python scripts/bootstrap_catalog.py --graph-path <temp-store>` on a fresh store, query the graph for `SELECT (COUNT(?c) AS ?n) WHERE { ?c a dec:Capability }` returns 13 (the PRD §5.2 catalog: classifier, code-writer, code-writer-heavy, standard-reasoning, standard-reasoning-frontier, deep-reasoning, mid-reasoning, fast-reasoning, vision-gui, vision-general, embedding, audio-transcribe, plus the FT-068 `verify-graph-author` row). Same for `dec:RoleBinding` returns 6 (the original five plus the FT-068 `verify-graph-author` binding).
2. **Content matches PRD.**
   - Each capability's `(capability_id, endpoint, model_identifier, tier, status, cost_currency)` tuple matches PRD §5.2 byte-for-byte.
   - Anthropic capabilities (`deep-reasoning`, `mid-reasoning`, `fast-reasoning`) carry `cost_cache_hit_per_m` and `cost_cache_write_5m`; Scaleway capabilities do not.
   - `standard-reasoning-frontier` carries `exposes_reasoning_trace = true`; all others false.
   - `standard-reasoning` carries `configurable_effort = true`; all others false.
   - `mid-reasoning` and `fast-reasoning` carry `status = "candidate"`.
   - Each binding's `(role_id, default_capability)` plus ordered escalation_steps matches PRD §6.2.
3. **Source hashes recorded.** Every Capability and RoleBinding artifact carries `dec:bootstrap_source = "<sha256 of yaml file>"`. The hash matches `sha256sum config/capabilities.yaml` (after canonical whitespace normalisation).
4. **Idempotency.** Run the bootstrap script a second time with unchanged YAML. Assert the graph state is unchanged (same artifact IRIs, no new versions, no rewrites). The script's exit code is 0; its output explicitly reports `"catalog: unchanged"` or `"capabilities skipped: 13, bindings skipped: 6, 0 divergent"`.
5. **Strict divergence handling.** Edit `config/capabilities.yaml` to change one `cost_input_per_m` value without bumping `version`. Re-run the script. Assert:
   - Exit code is **non-zero**.
   - Error output names the diverging artifact id (e.g. `code-writer@v1`) and shows the YAML-vs-graph diff.
   - **No writes occur** — the graph state is identical to before the re-run. Verify by re-querying counts and properties.
   - The error message hints at the resolution: either revert the YAML to match the graph, or bump the version field in YAML to create a new versioned artifact alongside.
6. **Ordering enforcement.** Edit `config/role-bindings.yaml` to reference a `default_capability` that doesn't exist in `config/capabilities.yaml`. Re-run the script (against a fresh store). Assert non-zero exit, error names `BootstrapError::UnresolvedReference`, **no writes occur** (capability writes that would have preceded the binding error are rolled back via the transaction).
7. **Bundle migration.** Pre-load a different fresh store with three `dec:Bundle` artifacts lacking `dec:stakes`. Run the bootstrap script with `--migrate`. Assert all three bundles now have `dec:stakes "routine"`. Run again with `--migrate`; assert no further changes.
8. **Session migration.** Same store has two existing Anthropic `dec:SessionRecord` artifacts lacking the new token-breakdown fields. Run the bootstrap script with `--migrate`. Assert each session now carries `input_tokens_base = <original input_tokens>`, `input_tokens_cache_write = 0`, `input_tokens_cache_hit = 0`. Re-run; assert idempotent.
9. **Atomicity on SHACL violation.** Corrupt the YAML (e.g. inject `endpoint: invalid`). Run the script. Assert the entire bootstrap is rolled back — no partial catalog in the graph, exit code non-zero, error message points at the offending YAML line and the SHACL constraint that failed.

⟦Σ:Types⟧{
  CatalogState ≜ ⟨capabilities:Set Capability, bindings:Set RoleBinding, sourceHashes:(Hash, Hash)⟩
  BootstrapOutcome ≜ Success(CatalogState) | Divergence(diff) | UnresolvedRef | ShaclViolation
}

⟦Γ:Invariants⟧{
  bootstrap(empty_store, y₁, y₂) = bootstrap(bootstrap(empty_store, y₁, y₂), y₁, y₂)   -- idempotent
  bootstrap(store, y_invalid) = error ∧ store_after = store_before                       -- atomic
  bootstrap(store_with_data, y_divergent) = Divergence(...) ∧ store_after = store_before -- strict no-overwrite
  ∀ a ∈ catalog: a.bootstrap_source ∈ {hash(y_capabilities), hash(y_bindings)}
  |catalog.capabilities| = 13 ∧ |catalog.bindings| = 6 (after first bootstrap of §5.2/§6.2 YAML)
}