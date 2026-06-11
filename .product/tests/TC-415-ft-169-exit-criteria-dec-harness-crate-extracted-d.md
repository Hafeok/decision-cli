---
id: TC-415
title: FT-169 exit criteria — dec-harness crate extracted, decision-cli reduced to wiring and slices, facades intact
type: exit-criteria
status: passing
validates:
  features: [FT-169]
  adrs: [ADR-086]
phase: 1
runner: bash
runner-args: scripts/checks/tc-415-dec-harness-extracted.sh
runner-timeout: 300
observes:
- exit-code
- stdout
- disk-state
last-run: 2026-06-11T14:06:27.760040469+00:00
last-run-duration: 4.3s
---

## Purpose

Exit criterion for [FT-169](FT-169) ([ADR-086](ADR-086)): the dec-harness extraction is only done when the crate exists, sits on `dec-graph` + `dec-ontology`, the full ADR-086 topology check passes hard with all three extracted crates present, the crate compiles standalone, and `decision-cli`'s core facades re-export it.

## Mechanism

Backed by `scripts/checks/tc-415-dec-harness-extracted.sh`, which asserts in order:

1. `crates/dec-harness/Cargo.toml` exists on disk.
2. The manifest declares dependencies on both `dec-graph` and `dec-ontology`.
3. `scripts/checks/crate-dependency-topology.sh` passes hard (exit-code 0 — by this point all three crates exist, so the full topology binds).
4. `crates/decision-cli/src/core/` contains a `pub use dec_harness` facade re-export.
5. `cargo check -p dec-harness` succeeds.

## Pass criteria

Observed surfaces: exit-code, stdout, and disk-state. Exit-code 0; stdout reports `OK: dec-harness extracted, topology intact, compiling, facades intact`.

## Fail criteria

Exit-code 1; stdout names the first failed criterion. Fails by design until FT-169 is implemented.