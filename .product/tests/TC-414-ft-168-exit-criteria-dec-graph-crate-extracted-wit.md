---
id: TC-414
title: FT-168 exit criteria — dec-graph crate extracted with the GraphWriter chokepoint, compiling, facades intact
type: exit-criteria
status: passing
validates:
  features: [FT-168]
  adrs: [ADR-086]
phase: 1
runner: bash
runner-args: scripts/checks/tc-414-dec-graph-extracted.sh
runner-timeout: 300
observes:
- exit-code
- stdout
- disk-state
last-run: 2026-06-11T13:46:50.631871209+00:00
last-run-duration: 0.2s
---

## Purpose

Exit criterion for [FT-168](FT-168) ([ADR-086](ADR-086)): the dec-graph extraction is only done when the crate exists, sits on `dec-ontology`, the workspace topology check passes hard, the crate compiles standalone, and `decision-cli`'s core facades re-export it.

## Mechanism

Backed by `scripts/checks/tc-414-dec-graph-extracted.sh`, which asserts in order:

1. `crates/dec-graph/Cargo.toml` exists on disk.
2. The manifest declares a dependency on `dec-ontology` (the domain sits beneath it).
3. `scripts/checks/crate-dependency-topology.sh` passes hard (exit-code 0, not the pre-migration exit 2 warning).
4. `crates/decision-cli/src/core/` contains a `pub use dec_graph` facade re-export.
5. `cargo check -p dec-graph` succeeds.

## Pass criteria

Observed surfaces: exit-code, stdout, and disk-state. Exit-code 0; stdout reports `OK: dec-graph extracted, topology intact, compiling, facades intact`.

## Fail criteria

Exit-code 1; stdout names the first failed criterion. Fails by design until FT-168 is implemented.