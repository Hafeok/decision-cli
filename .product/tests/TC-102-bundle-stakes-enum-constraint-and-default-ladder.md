---
id: TC-102
title: Bundle.stakes enum constraint and default ladder
type: exit-criteria
status: passing
validates:
  features:
  - FT-056
  adrs:
  - ADR-035
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test bundle_stakes
runner-timeout: 120
last-run: 2026-05-23T16:10:19.845721788+00:00
last-run-duration: 0.4s
---

## Description

Invariant: every `dec:Bundle` artifact carries exactly one `dec:stakes` literal in {`routine`, `elevated`, `foundational`} per [FT-056](FT-056) / [ADR-035](ADR-035). The default ladder in `core::bundle::default_stakes_for` is deterministic and matches the [ADR-035](ADR-035) §"Who sets it" table.

The runner is `cargo-test` and exercises:

1. **SHACL conformance.** Build bundles with each of the three valid stakes; assert SHACL passes. Build a bundle with `stakes = "critical"`; assert SHACL violation (enum). Build a bundle with `stakes` absent; assert SHACL violation (`sh:minCount 1`).
2. **Default ladder.** For a focal artifact of type `dec:Capability`, assert `default_stakes_for` returns `Foundational`. For a focal artifact of type `dec:RoleBinding`, assert `Foundational`. For a feature_spec linked to ≥ 2 cross-cutting ADRs, assert `Elevated`. For a feature_spec linked to 0 or 1 cross-cutting ADRs, assert `Routine`. For an unrecognised class, assert `Routine` (conservative default).
3. **Override path.** `core::bundle::BundleBuilder::with_stakes(Stakes::Foundational)` overrides the ladder; the resulting bundle's stakes is `Foundational` regardless of focal type.
4. **Migration idempotency.** Insert a bundle without stakes (simulating pre-PRD data), run `core::bootstrap::migrate_bundle_stakes`, assert the bundle now has `stakes = "routine"`. Run the migration again; assert no-op.

⟦Σ:Types⟧{
  Stakes ≜ routine | elevated | foundational
  Bundle ≜ ⟨hash:Hash, focal:IRI, stakes:Stakes, …⟩
}

⟦Γ:Invariants⟧{
  ∀ b:Bundle: shacl_conforms(b, BundleShape) ∧ b.stakes ∈ Stakes
  ∀ a:IRI: pure(default_stakes_for, a)
  ∀ b:Bundle: with_stakes(b, s).stakes = s
}