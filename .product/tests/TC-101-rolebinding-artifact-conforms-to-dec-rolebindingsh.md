---
id: TC-101
title: RoleBinding artifact conforms to dec:RoleBindingShape with ordered escalation steps
type: exit-criteria
status: passing
validates:
  features:
  - FT-055
  adrs:
  - ADR-034
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test role_binding_shape
runner-timeout: 120
last-run: 2026-05-24T19:14:23.673322616+00:00
last-run-duration: 0.3s
---

## Description

Invariant: every persisted `dec:RoleBinding` conforms to `dec:RoleBindingShape` ([FT-055](FT-055)), with strict ordering on `dec:escalation_steps`.

The shape constrains:

- `dec:role_id` (xsd:string, exactly one).
- `dec:default_capability` (object property, exactly one, range `dec:Capability`).
- `dec:escalation_steps` (optional; range is an `rdf:List` of `dec:EscalationStep`).
- `dec:version` (xsd:integer, exactly one, ≥ 1).
- `dec:active` (xsd:boolean, exactly one).
- A `sh:sparql` constraint enforcing at most one `dec:active = true` binding per `role_id`.
- A `sh:sparql` constraint enforcing `default_capability.status ≠ eol`.
- `dec:EscalationStepShape` requires `dec:step_capability` exactly once and `dec:triggers` with `sh:minCount 1`.
- `dec:EscalationTriggerShape` enforces `sh:in` over the [ADR-034](ADR-034) trigger vocabulary.

The runner is `cargo-test` and exercises:

1. Construct a binding identical to the PRD §6.2 `implementer` binding — including ordered escalation_steps `[code-writer-heavy → deep-reasoning]`. Assert SHACL passes. Round-trip read: assert the deserialised Rust value preserves step order.
2. Construct a binding with `escalation_steps = []`. Assert SHACL passes (bounded-classification roles).
3. Construct two `dec:RoleBinding` for the same `role_id` both with `active = true`. Assert SHACL violation.
4. Construct an `EscalationTrigger` with `trigger_signal = "stakes_critical"` (not in vocabulary). Assert SHACL violation.
5. Construct an `EscalationStep` with no triggers. Assert SHACL violation.

⟦Σ:Types⟧{
  RoleBinding ≜ ⟨role:String, default:CapabilityRef, steps:Ordered List EscalationStep, version:PosInt, active:Bool⟩
  EscalationStep ≜ ⟨capability:CapabilityRef, triggers:NonEmpty Set TriggerSignal⟩
}

⟦Γ:Invariants⟧{
  ∀ b:RoleBinding: shacl_conforms(b, RoleBindingShape)
  ∀ b₁,b₂:RoleBinding where b₁.active ∧ b₂.active: b₁.role = b₂.role ⇒ b₁ = b₂
  ∀ s:EscalationStep: |s.triggers| ≥ 1
  ∀ t:TriggerSignal: t ∈ closed_vocabulary
}