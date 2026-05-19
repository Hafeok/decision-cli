---
id: TC-020
title: decision-cli source tree matches ADR-016 vertical-slice layout
type: exit-criteria
status: failing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/vertical-slice-layout.sh
runner-timeout: 120
---

## Purpose

Exit criterion for [FT-018](FT-018). Verifies that after migration, the source tree under `crates/decision-cli/src/` matches the layout codified by [ADR-016](ADR-016).

## Steps

1. From the repo root, assert presence and shape of the expected directories:
   - `crates/decision-cli/src/lib.rs` exists.
   - `crates/decision-cli/src/main.rs` exists and is ≤ 250 lines.
   - `crates/decision-cli/src/core/mod.rs` exists.
   - Each of `ontology`, `vocab`, `bundled`, `scope`, `stream_writer`, `store`, `sparql` is present under `core/` (as either `<name>.rs` or `<name>/mod.rs`).
   - `crates/decision-cli/src/features/mod.rs` exists.
   - Each of `init`, `implement`, `health`, `events`, `session_inspect`, `finalize` is present under `features/` as `<name>/mod.rs`.
2. Assert absence of the legacy flat modules at `crates/decision-cli/src/`:
   - `init.rs`, `implement.rs`, `health.rs`, `events.rs`, `session_inspect.rs`, `finalize.rs`, `scope.rs`, `stream_writer.rs`, `bundled.rs`, `ontology.rs`, `vocab.rs` MUST NOT exist as direct children of `src/`.
3. Assert that `cargo build --workspace` succeeds.
4. Assert that `cargo test --workspace` is green (the existing TC suite is the regression gate).

## Pass criteria

All four assertions hold. Exit 0 = pass.

## Fail criteria

Any missing-or-extra layout entry, any compile failure, any failing test. Exit 1 with a message naming the offending entry / failure.

## Notes

This TC is the structural exit gate. Behavioural correctness is covered by the pre-existing slice-1 TCs (TC-001..TC-019), which this migration must leave untouched.
