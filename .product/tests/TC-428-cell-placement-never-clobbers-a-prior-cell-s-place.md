---
id: TC-428
title: Cell placement replaces the cell's own stale output at its resolved slot
type: invariant
status: passing
validates:
  features: [FT-170]
  adrs: [ADR-008, ADR-080]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_170_placement_replaces_stale_own_output
runner-timeout: 300
observes:
- exit-code
- stdout
- disk-state
last-run: 2026-06-11T18:55:52.916349280+00:00
last-run-duration: 0.8s
---

## Purpose

FT-170 invariant, amended by the first hardened FT-148 run: the resolved `output_path` is the cell's **own declared slot** (the registry guarantees distinct paths per cell), so pre-existing content there can only be the same cell's stale prior attempt — witnessed when a killed worker's partial `tests.rs` blocked its own replacement and failed the cluster. Placement replaces stale own-slot content (logged), and cross-cell protection remains structural via distinct resolved paths.

## Mechanism

`cargo test -p decision-cli ft_170_placement_replaces_stale_own_output`.

## Pass criteria

Observed surfaces: exit-code, stdout, disk-state. Exit-code 0 — the stale slot content is replaced by the fresh stray, which is moved (not copied).

## Fail criteria

Exit-code non-zero — stale own-slot content blocked the cell (the witnessed FT-148 failure mode), or the replacement mangled content.