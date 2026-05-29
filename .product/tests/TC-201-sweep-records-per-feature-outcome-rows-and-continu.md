---
id: TC-201
title: Sweep records per-feature outcome rows and continues past per-item failures
type: scenario
status: unimplemented
validates:
  features:
  - FT-111
  adrs: []
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_201_sweep_continues_past_per_item_failures
runner-timeout: 30
---

## Description

Per-item failure isolation per PAT-003. A single feature whose
per-feature drive returns an error must not abort the sweep —
the error is reified as `SweepOutcome::Error` and iteration
continues to the remaining features.

## Acceptance Criteria

Stub the per-feature driver so it returns `Ok(Done)` for
`FT-A` and `FT-C`, and `Err(SomeError)` for `FT-B`. Call the
sweep with features `[FT-A, FT-B, FT-C]`. Assert:

1. The returned `Vec<SweepRow>` has length 3.
2. Row 0 (`FT-A`) has outcome `Done`.
3. Row 1 (`FT-B`) has outcome `Error { detail: <displays the error> }`.
4. Row 2 (`FT-C`) has outcome `Done` — proves iteration continued.

The test does NOT spin up a real orchestration store; it stubs
the per-feature driver function pointer to make the failure
isolation property testable in milliseconds.
