---
id: TC-412
title: dec-ontology purity — domain crate declares no store, runtime, HTTP, CLI, or workspace dependencies
type: invariant
status: unimplemented
validates:
  features:
  - FT-167
  adrs:
  - ADR-086
phase: 1
runner: bash
runner-args: scripts/checks/dec-ontology-purity.sh
runner-timeout: 30
observes:
- exit-code
- stdout
---

## Purpose

Fitness check for [ADR-086](ADR-086)'s central contract: `dec-ontology` is pure data. The domain crate's trustworthiness comes from what its dependency tree structurally cannot do — open a store, spawn a runtime, make a network call, parse CLI args. A single `tokio = …` line in its manifest silently re-monolithizes the center; this TC makes that line a red PR.

## Mechanism

Backed by `scripts/checks/dec-ontology-purity.sh`. The script greps `crates/dec-ontology/Cargo.toml` for declarations of any forbidden crate: `oxigraph`, `tokio`, `axum`, `reqwest`, `clap`, `anyhow`, `oxi-events`, `decision-cli`, `product-core`, `dec-graph`, `dec-harness`.

## Pass criteria

Observed surfaces: the script's exit-code and its stdout diagnostics. Exit-code 0: the manifest declares none of the forbidden crates (allowed set per ADR-086: `oxrdf`, `serde`, `serde_json`, `thiserror`, `chrono`, `uuid`); stdout reports `OK: dec-ontology dependency tree is pure`.

## Fail criteria

Exit-code 1; stdout names the forbidden dependency.

## Notes

- Exit 2 (warning) while `crates/dec-ontology` does not yet exist ([FT-167](FT-167) pending).
- Cross-cutting via `validates.adrs: [ADR-086]`; runs on every PR through `product verify --platform` ([ADR-014](ADR-014)).
- The check reads the declared manifest, not the resolved lockfile — transitive purity follows because the allowed crates are themselves IO-free at the model level.

## Formal Specification

⟦Σ:Types⟧{
  Dep ≜ String
  Forbidden ≜ {oxigraph, tokio, axum, reqwest, clap, anyhow, oxi-events, decision-cli, product-core, dec-graph, dec-harness}
}

⟦Γ:Invariants⟧{
  ∀ d:Dep ∈ manifest(dec-ontology): d ∉ Forbidden
}