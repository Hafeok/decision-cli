---
id: TC-413
title: FT-167 exit criteria — dec-ontology crate extracted, pure, compiling, with core facades intact
type: exit-criteria
status: passing
validates:
  features: [FT-167]
  adrs: [ADR-086]
phase: 1
runner: bash
runner-args: scripts/checks/tc-413-dec-ontology-extracted.sh
runner-timeout: 300
observes:
- exit-code
- stdout
- disk-state
last-run: 2026-06-11T13:27:53.033915894+00:00
last-run-duration: 0.2s
---

## Purpose

Exit criterion for [FT-167](FT-167) ([ADR-086](ADR-086)): the dec-ontology extraction is only done when the crate exists, is pure, compiles standalone, and `decision-cli` re-exports it through the `core::ontology` / `core::vocab` facades so no feature-slice import changed.

## Mechanism

Backed by `scripts/checks/tc-413-dec-ontology-extracted.sh`, which asserts in order (disk-state, then sub-checks, then compilation):

1. `crates/dec-ontology/Cargo.toml` exists on disk.
2. `scripts/checks/dec-ontology-purity.sh` passes hard (exit-code 0, not the pre-migration exit 2 warning).
3. `crates/decision-cli/src/core/` contains a `pub use dec_ontology` facade re-export.
4. `cargo check -p dec-ontology` succeeds.

## Pass criteria

Observed surfaces: exit-code, stdout, and disk-state. Exit-code 0; stdout reports `OK: dec-ontology extracted, pure, compiling, facades intact`.

## Fail criteria

Exit-code 1; stdout names the first failed criterion. Fails by design until FT-167 is implemented.