---
id: TC-140
title: 'decision-cli: verify-graph-author capability binding (endpoint + model_id on the bundle) — exit criterion'
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_140_verify_graph_author_capability_binding
runner-timeout: 120
last-run: 2026-05-25T19:09:58.308776805+00:00
last-run-duration: 0.5s
---

## Description

Exit criterion for [FT-068](FT-068). Validates that the verify-graph-author
worker's bundle envelope carries `endpoint`, `model_id`, `parameters`, and
`max_tokens` resolved through the capability layer (FT-061 / ADR-033 /
ADR-037), rather than being hardcoded inside the worker.

The test exercises four acceptance shapes:

1. **`dec init` seeds the binding.** A fresh init seeds the
   `verify-graph-author` role + `dec:Capability` + `dec:RoleBinding` so
   `core::dispatch::capability_resolver::resolve_default_capability(
   store, "verify-graph-author")` returns a Scaleway-hosted capability
   (`qwen3-coder-30b-a3b-instruct`) consistent with ADR-037 without any
   operator step.

2. **Bundle plumbing through `dec verify graph generate`.** Running the
   generate handler against a feature with uncovered TCs (matcher
   returns `Partial` / `None`) exercises `assemble_bundle` with the
   resolved capability. A mocked worker captures the bundle; the
   resulting `VerifyGraphAuthorInputJson` carries
   `endpoint = "scaleway"`, `model_id = "qwen3-coder-30b-a3b-instruct"`,
   `parameters = {}`, and `max_tokens = 32_000`.

3. **Hash covers the new fields.** `compute_bundle_hash` reflects
   changes to `endpoint`, `model_id`, and `max_tokens` — proving the
   FT-068 §Invariants claim "changing the capability changes the hash"
   holds. Identical bundles produce identical hashes; mutating any of
   the three new content-bearing fields produces a different hash.

4. **Resolver error path uses `capability:` prefix.** Calling the
   resolver against an un-seeded store returns
   `ResolverError::NoActiveBinding { role_id: "verify-graph-author" }`;
   the FT-068 handler maps this to `HandlerError::Internal` whose
   `detail` starts with `capability:` per the FT-061 operator-messaging
   convention.

## Acceptance

Runner: `cargo test -p decision-cli --test
tc_140_verify_graph_author_capability_binding`. All four `#[test]`s pass.

## Formal specification

⟦Σ:Types⟧{
  Bundle ≜ VerifyGraphAuthorInputJson
  Resolved ≜ ResolvedCapability
}

⟦Γ:Invariants⟧{
  bundle.endpoint = resolved.endpoint.as_str
  bundle.model_id = resolved.model_identifier
  bundle.max_tokens = clamp(resolved.max_output, 256, 64_000)
  bundle.parameters = {}
  ∀ b₁,b₂:Bundle: (b₁.endpoint ≠ b₂.endpoint ∨ b₁.model_id ≠ b₂.model_id
                   ∨ b₁.max_tokens ≠ b₂.max_tokens) ⇒ hash(b₁) ≠ hash(b₂)
  resolve(empty_store, "verify-graph-author") = NoActiveBinding
  map(NoActiveBinding) = Internal{detail ~ /^capability:/}
}