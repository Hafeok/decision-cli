---
id: TC-100
title: Capability artifact conforms to dec:CapabilityShape SHACL
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-054
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test capability_shape
runner-timeout: 120
---

## Description

Invariant: every persisted `dec:Capability` artifact conforms to the SHACL shape defined in [FT-054](FT-054): `dec:CapabilityShape`. The shape constrains:

- `dec:capability_id` (xsd:string, exactly one).
- `dec:endpoint` ∈ {`scaleway`, `anthropic`} (exactly one).
- `dec:model_identifier` (xsd:string, exactly one).
- `dec:tier` (xsd:integer, optional; values outside 0–3 forbidden for non-specialty capabilities).
- `dec:context_window`, `dec:max_output` (xsd:integer, exactly one each, non-negative).
- `dec:supports_vision`, `dec:supports_tool_calling` (xsd:boolean, exactly one each).
- `dec:cost_input_per_m`, `dec:cost_output_per_m` (xsd:decimal, exactly one each, non-negative).
- `dec:configurable_effort` (xsd:boolean, optional; default false).
- `dec:status` ∈ {`active`, `preview`, `eol`, `candidate`} (exactly one).
- `dec:version` (xsd:integer, exactly one, ≥ 1).
- A `sh:sparql` constraint enforcing `(capability_id, version)` uniqueness within `status = active`.

The runner is a `cargo-test` integration that loads:

1. A sample of every PRD §5.2 capability constructed in Turtle.
2. A battery of constructed-invalid capabilities — missing required field, bad enum value, negative cost, duplicate `(id, version)` in active status, tier outside 0–3.

Runs `oxigraph::shacl::validate` and asserts the valid ones pass and the invalid ones produce specific shape-violation reports matching the constraint that failed.

⟦Σ:Types⟧{
  Capability ≜ ⟨id:Tag, endpoint:Endpoint, model:String, tier:Maybe Int, ctx:Nat, out:Nat, vision:Bool, tools:Bool, costIn:NonNegDec, costOut:NonNegDec, effort:Bool, status:Status, version:PosInt⟩
  Endpoint ≜ scaleway | anthropic
  Status ≜ active | preview | eol | candidate
}

⟦Γ:Invariants⟧{
  ∀ c:Capability: shacl_conforms(c, CapabilityShape)
  ∀ c₁,c₂:Capability where c₁.status = active ∧ c₂.status = active: (c₁.id, c₁.version) = (c₂.id, c₂.version) ⇒ c₁ = c₂
  ∀ c:Capability: c.costIn ≥ 0 ∧ c.costOut ≥ 0
}
