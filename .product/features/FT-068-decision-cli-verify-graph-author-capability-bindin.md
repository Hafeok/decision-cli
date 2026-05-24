---
id: FT-068
title: 'decision-cli: verify-graph-author capability binding (endpoint + model_id on the bundle)'
phase: 3
status: planned
depends-on: [FT-058, FT-061, FT-067]
adrs: [ADR-008, ADR-020, ADR-033, ADR-037]
tests: []
domains: []
domains-acknowledged: {}
---

## Description

The verify-graph-author worker's bundle envelope (post-FT-064 source) requires `endpoint` and `model_id` at the top level, but the Rust assembler in `crates/decision-cli/src/features/verify_graph_generate/bundle.rs` does not populate them. Result: with a freshly-installed worker, `dec verify graph generate` aborts during Pydantic validation with `model_id: Field required`. Without it, an old stale install silently defaulted to Anthropic Sonnet 4.5 — exactly the hardcoded binding ADR-037 / FT-064 set out to remove.

FT-061 / FT-062 already wired graph-driven capability resolution for `code-writer` and `verifier`. FT-068 extends the same path to `verify-graph-author` so the verify-graph proposal flow respects ADR-037 (Scaleway as default for cost-dominant roles) and ADR-033 (capability-based routing). After this lands, the dogfood pass over the 61 uncovered features (the parallel "write verify for everything" effort) runs end-to-end against a live model.

This feature_spec covers only the *binding wiring* — the role catalog entry, the RoleBinding + Capability seeds, and the bundle plumbing. The CLI surface of `dec verify graph generate` is unchanged.

## Functional Specification

### Inputs

- The capability and role-binding catalog seeded at `dec init` (FT-058) — extended here with a `verify-graph-author` row.
- `core::dispatch::capability_resolver::resolve_default_capability` (FT-061) — unchanged; this feature is one new caller.
- The existing `VerifyGraphAuthorInputJson` bundle struct — extended with four new fields.
- The Python worker's `VerifyGraphAuthorInput` shape (post-FT-064), which requires `model_id` and defaults `endpoint` to `"anthropic"`.

### Outputs

- A new role catalog seed `crates/decision-cli/src/core/role_catalog/seeds/verify-graph-author.ttl` referencing a `default_capability` IRI.
- A new RoleBinding row in `config/role-bindings.yaml` (or wherever FT-058 reads bindings from) pointing `verify-graph-author` at a `dec:Capability` consistent with ADR-037: Scaleway DeepSeek or comparable cost-dominant model when available, falling back to a documented Anthropic capability when not.
- `assemble_bundle` in `features/verify_graph_generate/bundle.rs` accepts a `ResolvedCapability` and writes:
  ```rust
  pub endpoint: String,                  // "scaleway" | "anthropic"
  pub model_id: String,
  pub parameters: serde_json::Value,     // capability-resolved params (currently {})
  pub max_tokens: u32,
  ```
- The handler at `features/verify_graph_generate/mod.rs` calls `resolve_default_capability("verify-graph-author", &store)` before `assemble_bundle`, surfaces `ResolverError` through the same `HandlerError::Internal` channel FT-061 uses, and passes the resolved capability into the assembler.

### State

- `dec init` (fresh stores) seeds the new role binding alongside the existing `code-writer` and `verifier` bindings.
- Existing stores: the catalog bootstrap path (FT-058) is already idempotent for added rows — `dec init --reseed` (or a one-off migration) registers the new binding without disturbing existing artifacts.
- No on-disk artifact format changes outside the catalog seed files.

### Behaviour

1. Author the role catalog seed:
   ```turtle
   <https://decision-cli.dev/ns/role/verify-graph-author>
       a                     dec:Role ;
       dec:roleId            "verify-graph-author" ;
       dec:roleInputType     <https://decision-cli.dev/ns#FeatureSpec> ,
                             <https://decision-cli.dev/ns#TestCriterion> ,
                             <https://decision-cli.dev/ns#VerificationEnvironment> ;
       dec:roleOutputType    <https://decision-cli.dev/ns#GraphProposal> .
   ```
   No `roleModelBinding` literal — capabilities own this now (ADR-033).
2. Author the role-binding row pointing at a `dec:Capability` whose endpoint defaults to `scaleway` per ADR-037. The capability artifact must satisfy the same SHACL constraints existing capabilities do (FT-054); pick `cost_cache_hit_per_m` only if the chosen model supports caching per FT-065.
3. Extend `VerifyGraphAuthorInputJson` (Rust) with `endpoint`, `model_id`, `parameters`, `max_tokens` fields — names and types matching the Python pydantic shape byte-for-byte. Update `compute_bundle_hash` so the hash covers the new canonical shape (mocked-bundle test fixtures need their hashes regenerated alongside).
4. Update `assemble_bundle`:
   ```rust
   pub fn assemble_bundle(
       workdir: &Path,
       product_root: &Path,
       feature_id: &str,
       env_short: &str,
       match_report: &MatchReport,
       capability: &ResolvedCapability,   // NEW
   ) -> Result<VerifyGraphAuthorInputJson, HandlerError>
   ```
5. Update the handler at `features/verify_graph_generate/mod.rs` to call `resolve_default_capability("verify-graph-author", store)` immediately before `assemble_bundle`. Map `ResolverError` to `HandlerError::Internal { detail: "capability: …" }` so operator messaging matches FT-061's convention.
6. No CLI surface change. Operators who want to override the resolved capability today can supersede the binding via the existing catalog edit path (slice-2.5); a `--capability` override flag is deferred.

### Invariants

- The bundle hash is deterministic given the same `(feature, env, candidate_graphs, resolved capability)` tuple. Changing the capability changes the hash — by design, since the proposal a Scaleway model produces is not interchangeable with the proposal an Anthropic model produces.
- ADR-037: the resolved default endpoint is `scaleway` whenever a Scaleway-hosted capability satisfying the role's compatibility constraints exists; Anthropic only when Scaleway is unavailable.
- ADR-008: the worker remains stateless. It does not query the catalog itself — it consumes `(endpoint, model_id, parameters)` verbatim from the bundle.
- `resolve_default_capability` is called exactly once per generate-request; the result is recorded on the proposal session for reproducibility (matches FT-061's pattern for code-writer dispatches).

### Error handling

- `ResolverError::NoActiveBinding` (the bootstrap migration was skipped) → `HandlerError::Internal { detail: "capability: verify-graph-author has no active binding; run `dec init --reseed`" }`. Exit 1.
- `ResolverError::UnknownCapability` (corrupted catalog) → `HandlerError::Internal` with the offending IRI; operator runs `dec init --reseed` or supersedes the binding.
- Worker subprocess errors → unchanged path through FT-067's resolver chain.

### Boundaries

- **In scope.** Role catalog seed, role-binding row, `assemble_bundle` signature change, `VerifyGraphAuthorInputJson` shape extension, handler call to `resolve_default_capability`, test fixtures for the four new bundle fields, one new TC.
- **Out of scope.** A `--capability` CLI override (deferred). Escalation for verify-graph-author (FT-062 covers verifier escalation; the verify-graph-author role does not yet have an escalation policy and ADR-030 §7 keeps Level-3 review the only failure mode). Asymmetric model selection across feature classes (Phase B). Renaming or reshaping the worker's pydantic model.

## Out of scope

- A `--model` / `--endpoint` operator override on `dec verify graph generate`.
- Re-running the matcher when the capability changes (the matcher does not consume capability info).
- Backfilling existing `dec:Session` records with capability version pins (historical truth is correct as-is).

## References

- [FT-058](FT-058) — capability and role-binding catalog bootstrap.
- [FT-061](FT-061) — dispatcher capability resolution (default-capability path).
- [FT-064](FT-064) — migration cleanup that removed hardcoded model bindings from the worker layer; FT-068 is the dependency back-paying that migration's tab for the verify-graph-author role.
- [FT-067](FT-067) — worker resolver routing; precondition for live dogfood.
- [ADR-033](ADR-033) — capability-based model routing as a graph layer.
- [ADR-037](ADR-037) — Scaleway as default endpoint for cost-dominant roles.
- [ADR-020](ADR-020) — single-LLM-call worker contract (Phase A binding constraint).
