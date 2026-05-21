---
id: TC-021
title: no cross-feature imports inside decision-cli features
type: invariant
status: failing
validates:
  features: []
  adrs:
  - ADR-016
phase: 1
runner: bash
runner-args: scripts/checks/vertical-slice-imports.sh
runner-timeout: 60
last-run: 2026-05-21T15:21:18.505743289+00:00
last-run-duration: 0.2s
failure-message: ""
---

## Purpose

Invariant check for [ADR-016](ADR-016). The vertical-slice layout makes cross-feature coupling structurally impossible at compile time, but the audit script catches anyone who routes around module privacy via the crate root (e.g. `use crate::features::implement::ImplementOutcome` from inside `features::events`).

## Mechanism

Backed by `scripts/checks/vertical-slice-imports.sh`. The script:

1. Walks every `*.rs` file under `crates/decision-cli/src/features/*/`.
2. For each file, infers which feature it belongs to from its path (`features/<F>/...`).
3. Greps for `use crate::features::<G>::` and `use super::super::<G>::` where `<G> ≠ <F>`.
4. Also flags wildcard reaches like `use crate::features::*` from inside a feature directory.
5. Exits 1 on the first violation with `file:line` and the offending `use` line printed. Exits 0 on a clean tree.

## Pass criteria

`scripts/checks/vertical-slice-imports.sh` exits 0 against the live tree.

## Fail criteria

Exit code ≠ 0; the violation message names the offending file, line, and `use` statement so the author can promote the shared code into `core/` per the ADR-016 promotion rule.

## Notes

- This TC is `scope: cross-cutting` and `validates.adrs: [ADR-016]` so it runs on every PR via `product verify --platform` (per [ADR-014](ADR-014)).
- The check script lives alongside the other ADR-014 fitness functions (`scripts/checks/file-length.sh`, etc.) and is governed by `source-files` on ADR-016.

## Formal Specification

⟦Σ:Types⟧{
  Feature ≜ IRI
  UseStmt ≜ ⟨file:Path, line:Int, target_feature:Feature⟩
}

⟦Γ:Invariants⟧{
  ∀ u:UseStmt ∈ crates/decision-cli/src/features/:
    feature_of(u.file) = u.target_feature
}