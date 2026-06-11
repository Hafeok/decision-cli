---
id: TC-425
title: Cell placement relocates a single stray output to the resolved output_path
type: exit-criteria
status: passing
validates:
  features: [FT-170]
  adrs: [ADR-008, ADR-080]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_170_placement_relocates_single_stray
runner-timeout: 300
observes:
- exit-code
- stdout
- disk-state
last-run: 2026-06-11T18:55:52.916349280+00:00
last-run-duration: 140.3s
---

## Purpose

FT-170 case 2 — the core fix for the witnessed FT-147 path drift: when the worker writes its artifact at a path of its own invention, the harness relocates it to the resolved `output_path` (content preserved, stray removed, drift logged for prompt tuning).

## Mechanism

`cargo test -p decision-cli ft_170_placement_relocates_single_stray` — exercises `place_cell_output` with a before/after sandbox snapshot where one right-kind file landed under a worker-invented nested dir.

## Pass criteria

Observed surfaces: exit-code, stdout, disk-state. Exit-code 0 — the artifact sits at the resolved path with identical content and the stray no longer exists (moved, not copied).

## Fail criteria

Exit-code non-zero — placement failed to relocate, copied instead of moved, or altered content.