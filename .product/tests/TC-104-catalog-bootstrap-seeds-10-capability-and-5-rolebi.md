---
id: TC-104
title: Catalog bootstrap seeds 10 Capability and 5 RoleBinding artifacts idempotently
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-058
  adrs: []
phase: 2
runner: bash
runner-args: tests/scripts/tc-104-catalog-bootstrap.sh
runner-timeout: 180
---

## Description

Scenario: `dec init` against an empty orchestration store seeds the catalog from `crates/decision-cli/seeds/capabilities.yaml` and `seeds/role_bindings.yaml` per [FT-058](FT-058) / [ADR-036](ADR-036), producing exactly 10 `dec:Capability` and 5 `dec:RoleBinding` artifacts. A second `dec init` against the same store with unchanged YAML is a no-op.

The runner is `bash` driving a `cargo run` of `dec init` against a temp store, asserting via `dec capability list` / `dec binding list` (or direct SPARQL via `oxigraph::sparql`) the expected counts and contents.

Acceptance:

1. **First-bootstrap counts.** After `dec init --from streams/decision-cli-development.ttl` on a fresh store, query the graph for `SELECT (COUNT(?c) AS ?n) WHERE { ?c a dec:Capability }` returns 10. Same for `dec:RoleBinding` returns 5.
2. **Content matches PRD.** Each capability's `(capability_id, endpoint, model_identifier, tier, status)` tuple matches PRD §5.2 byte-for-byte. Each binding's `(role_id, default_capability)` plus ordered escalation_steps matches PRD §6.2.
3. **Source hashes recorded.** Every Capability and RoleBinding artifact carries `dec:bootstrap_source = "<sha256 of yaml file>"`. The hash matches `sha256sum crates/decision-cli/seeds/capabilities.yaml` (after canonical whitespace normalisation).
4. **Idempotency.** Run `dec init --from …` a second time with unchanged YAML. Assert the graph state is unchanged (same artifact IRIs, no new versions, no rewrites). The init log should explicitly report "catalog: unchanged".
5. **Bundle migration.** Pre-load the store with three `dec:Bundle` artifacts lacking `dec:stakes` (simulating pre-PRD data). Run `dec init`. Assert all three bundles now have `dec:stakes "routine"`. Run init again; assert no further changes.
6. **Atomicity on SHACL violation.** Corrupt the YAML (e.g. inject `endpoint: invalid`). Run `dec init`. Assert the entire bootstrap is rolled back — no partial catalog in the graph, exit code non-zero, error message points at the offending YAML line.

⟦Σ:Types⟧{
  CatalogState ≜ ⟨capabilities:Set Capability, bindings:Set RoleBinding, sourceHashes:(Hash, Hash)⟩
}

⟦Γ:Invariants⟧{
  bootstrap(empty_store, y₁, y₂) = bootstrap(bootstrap(empty_store, y₁, y₂), y₁, y₂)   -- idempotent
  bootstrap(store, y_invalid) = error ∧ store_after = store_before                       -- atomic
  ∀ a ∈ catalog: a.bootstrap_source ∈ {hash(y_capabilities), hash(y_bindings)}
}
