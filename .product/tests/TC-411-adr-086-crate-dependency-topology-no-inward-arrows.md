---
id: TC-411
title: ADR-086 crate dependency topology — no inward arrows violated across the workspace
type: invariant
status: passing
validates:
  features:
  - FT-167
  - FT-168
  - FT-169
  adrs:
  - ADR-086
phase: 1
runner: bash
runner-args: scripts/checks/crate-dependency-topology.sh
runner-timeout: 30
observes:
- exit-code
- stdout
last-run: 2026-06-11T13:46:50.631871209+00:00
last-run-duration: 0.0s
---

## Purpose

Fitness check for [ADR-086](ADR-086). The stable-dependency crate topology (`oxrdf ← dec-ontology ← dec-graph ← dec-harness ← decision-cli`) is enforced primarily by Cargo, but a crate could still declare a legal-but-forbidden edge (e.g. `dec-graph` depending on `clap`, or a library crate depending on the `decision-cli` binary crate). This TC audits every workspace manifest for forbidden edges.

## Mechanism

Backed by `scripts/checks/crate-dependency-topology.sh`. The script:

1. Asserts no crate other than `decision-cli` itself declares a dependency on `decision-cli`.
2. Asserts `dec-ontology` declares no workspace crate (`dec-graph`, `dec-harness`, `oxi-events`, `product-core`).
3. Asserts `dec-graph` declares neither `dec-harness` nor `clap`.
4. Asserts `dec-harness` declares no `clap`.

## Pass criteria

Observed surfaces: the script's exit-code and its stdout diagnostics. Exit-code 0 against the live tree with all three extracted crates present; stdout reports `OK: ADR-086 crate dependency topology is intact`.

## Fail criteria

Exit-code 1; stdout names the offending manifest and dependency.

## Notes

- Exit 2 (warning) while `crates/dec-ontology` / `dec-graph` / `dec-harness` do not yet exist ([FT-167](FT-167)–[FT-169](FT-169) pending) — the topology is not yet binding, surfaced but non-blocking per the ADR-013 runner contract.
- This TC is cross-cutting via `validates.adrs: [ADR-086]` and runs on every PR through `product verify --platform` (per [ADR-014](ADR-014)).
- Complements [TC-021](TC-021) (intra-crate slice isolation, ADR-016) and the ADR-001 boundary check — same principle, one level per check.

## Formal Specification

⟦Σ:Types⟧{
  Crate ≜ String
  Edge ≜ ⟨from:Crate, to:Crate⟩
}

⟦Γ:Invariants⟧{
  ∀ e:Edge ∈ workspace_manifests:
    e.to ≠ "decision-cli"
    ∧ (e.from = "dec-ontology" → e.to ∉ WorkspaceCrates)
    ∧ (e.from = "dec-graph" → e.to ∉ {"dec-harness", "clap"})
    ∧ (e.from = "dec-harness" → e.to ≠ "clap")
}