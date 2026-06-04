---
id: TC-311
title: FT-131 --no-author run produces FT-119 byte-for-byte parity on classifications and Stuck reasons
type: scenario
status: passing
validates:
  features:
  - FT-131
  adrs:
  - ADR-076
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test ft_131_no_author_parity
runner-timeout: 120
observes:
- exit-code
- stdout
last-run: 2026-06-04T09:35:01.130813997+00:00
last-run-duration: 0.1s
---

## Purpose

Validates FT-131 (FeatureReadyPlanner) against ADR-076's `--no-author` opt-out invariant. Running `dec drive def-ready FT-XXX --no-author` must produce classification text and Stuck-reason strings byte-for-byte identical to FT-119's planner output for every fixture covered by TC-253..TC-258. This is the regression guard that lets operators keep using FT-119's read-only readiness view when they explicitly opt out of authoring.

## Acceptance

- For each fixture in the FT-119 golden set (TC-253 through TC-258), running FT-131 with `--no-author` produces stdout identical (byte-for-byte) to the FT-119 reference output.
- The Action enum's Display impl renders the same strings as FT-119's Action under --no-author.
- No dispatch is recorded by the stub harness under --no-author (the author paths are inert).
- The test diffs actual vs golden byte-by-byte and fails with the first diverging fixture id and offset.
- The test exits with status 0.

## Inputs

The TC-253..TC-258 fixture set reused as golden files (the FT-119 planner output is captured into `tests/fixtures/ft_119_golden/`). The FT-131 planner is constructed with `no_author=true`. The test loads each fixture's input state into a synthetic store and compares the rendered output to the corresponding golden file.

## Out of scope

- Authoring-enabled paths (covered by TC-308 / TC-309 / TC-310).
- New classification rows added by FT-131 that have no FT-119 analogue (those rows are excluded from the parity diff).