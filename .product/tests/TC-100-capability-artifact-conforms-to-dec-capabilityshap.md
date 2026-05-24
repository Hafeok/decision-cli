---
id: TC-100
title: Capability artifact conforms to dec:CapabilityShape SHACL
type: exit-criteria
status: passing
validates:
  features:
  - FT-054
  adrs:
  - ADR-033
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test capability_shape
runner-timeout: 120
last-run: 2026-05-24T19:14:23.673322616+00:00
last-run-duration: 0.2s
---

## Description

Invariant: every persisted `dec:Capability` artifact conforms to the SHACL shape defined in [FT-054](FT-054): `dec:CapabilityShape`. The shape constrains:

- `dec:capability_id` (xsd:string, exactly one).
- `dec:endpoint` ∈ {`scaleway`, `anthropic`} (exactly one).
- `dec:model_identifier` (xsd:string, exactly one).
- `dec:tier` (xsd:integer, optional; values outside 0–3 forbidden for non-specialty / non-candidate capabilities).
- `dec:context_window`, `dec:max_output` (xsd:integer, exactly one each, non-negative).
- `dec:supports_vision`, `dec:supports_tool_calling` (xsd:boolean, exactly one each).
- `dec:cost_input_per_m`, `dec:cost_output_per_m` (xsd:decimal, exactly one each, non-negative).
- `dec:cost_cache_hit_per_m`, `dec:cost_cache_write_5m` (xsd:decimal, optional, non-negative; **paired** — either both present or both absent, enforced by `sh:sparql`).
- `dec:cost_currency` ∈ {`EUR`, `USD`} (exactly one).
- `dec:configurable_effort` (xsd:boolean, optional; default false).
- `dec:exposes_reasoning_trace` (xsd:boolean, optional; default false).
- `dec:status` ∈ {`active`, `preview`, `eol`, `candidate`} (exactly one).
- `dec:version` (xsd:integer, exactly one, ≥ 1).
- A `sh:sparql` constraint enforcing `(capability_id, version)` uniqueness within `status = active`.
- A `sh:sparql` constraint enforcing the cache-cost pair invariant: `cost_cache_hit_per_m` is set iff `cost_cache_write_5m` is set.

The runner is a `cargo-test` integration that loads:

1. A sample of every PRD §5.2 capability constructed in Turtle (12 entries).
2. A battery of constructed-invalid capabilities — missing required field, bad enum value, negative cost, duplicate `(id, version)` in active status, tier outside 0–3, half-set cache-cost pair (only `cost_cache_hit_per_m` without `cost_cache_write_5m`), invalid currency string.

Runs `oxigraph::shacl::validate` and asserts the valid ones pass and the invalid ones produce specific shape-violation reports matching the constraint that failed.

**Specific assertions per PRD §5.2:**

- `code-writer` (Scaleway, qwen3-coder-30b): cost_input_per_m=0.20, cost_output_per_m=0.80, cost_currency=EUR, configurable_effort=false, exposes_reasoning_trace=false, both cache fields absent. SHACL passes.
- `standard-reasoning` (Scaleway, gpt-oss-120b): configurable_effort=true, exposes_reasoning_trace=false. SHACL passes.
- `standard-reasoning-frontier` (Scaleway, qwen3.5-397b): exposes_reasoning_trace=**true**. SHACL passes.
- `deep-reasoning` (Anthropic, opus-4-7): cost_input_per_m=5.00, cost_output_per_m=25.00, cost_cache_hit_per_m=0.50, cost_cache_write_5m present, cost_currency=USD. SHACL passes.
- `mid-reasoning` and `fast-reasoning` (Anthropic candidates, status=`candidate`): SHACL passes; status enum accepts `candidate`.
- An invalid capability missing `cost_currency`: SHACL violation (`sh:minCount`).
- An invalid capability with `cost_currency = "GBP"`: SHACL violation (`sh:in`).
- An invalid capability with `cost_cache_hit_per_m = 0.50` but no `cost_cache_write_5m`: SHACL `sh:sparql` violation on the cache-cost pair constraint.

⟦Σ:Types⟧{
  Capability ≜ ⟨id:Tag, endpoint:Endpoint, model:String, tier:Maybe Int, ctx:Nat, out:Nat, vision:Bool, tools:Bool, costIn:NonNegDec, costOut:NonNegDec, costCacheHit:Maybe NonNegDec, costCacheWrite:Maybe NonNegDec, currency:Currency, effort:Bool, trace:Bool, status:Status, version:PosInt⟩
  Endpoint ≜ scaleway | anthropic
  Currency ≜ EUR | USD
  Status ≜ active | preview | eol | candidate
}

⟦Γ:Invariants⟧{
  ∀ c:Capability: shacl_conforms(c, CapabilityShape)
  ∀ c:Capability: c.costCacheHit ≠ ⊥ ⇔ c.costCacheWrite ≠ ⊥
  ∀ c₁,c₂:Capability where c₁.status = active ∧ c₂.status = active: (c₁.id, c₁.version) = (c₂.id, c₂.version) ⇒ c₁ = c₂
  ∀ c:Capability: c.costIn ≥ 0 ∧ c.costOut ≥ 0
}