---
id: TC-257
title: dec drive def-ready --all aggregates per-feature outcomes into the FT-111 SweepTally and exit codes match the sweep contract
type: scenario
status: unimplemented
validates:
  features:
  - FT-119
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::ft_119_drive_def_ready::dispatch_tests::tc_257
runner-timeout: 120
observes:
- stdout
- exit-code
---

## Claim

`dec drive def-ready --all` enumerates every feature_spec, drives each one
under the per-feature timeout, and aggregates the results into the same
`SweepRow` / `SweepTally` shapes FT-111 ships. The exit code is `0` iff
every feature ended `Done`, otherwise `1`. `--filter` restricts the set;
`--format json|tsv|text` selects the renderer.

## Scenarios

### Setup

Seed a fixture product graph with five features:

| Id | Inspector configuration |
|---|---|
| FT-A | DoR row: ready (→ `Done`) |
| FT-B | DoR row: VGA-dispatched then ready (→ `Done`) |
| FT-C | DoR row: preflight warnings (→ `Stuck`) |
| FT-D | DoR row: no TCs (→ `Stuck`) |
| FT-E | injected inspector error (→ `Error`) |

### Test A — full sweep

Run `dec drive def-ready --all --per-feature-timeout 10 --max-iter 4 --format
json`.

Assertions:

1. Exit code is `1` (FT-C, FT-D, FT-E are not Done).
2. JSON output deserialises to `{ rows: [...], tally: {...} }`.
3. `rows.len() == 5`; row order is `FT-A, FT-B, FT-C, FT-D, FT-E`
   (numeric-suffix ascending, same as FT-111).
4. Tally:
   `done: 2, stuck: 2, hit_max_iter: 0, timeout: 0, error: 1`.
5. The Stuck rows carry the reason verbatim from the planner.

### Test B — `--filter`

Run `dec drive def-ready --all --filter FT-A,FT-C --format tsv`.

Assertions:

1. Exit code is `1`.
2. `rows.len() == 2`; order is `FT-A, FT-C`.
3. TSV header matches FT-111's exactly with one cell adjusted: the goal
   column reads `def-ready`.

### Test C — unknown filter id

Run `dec drive def-ready --all --filter FT-A,FT-NOT-REAL`.

Assertions:

1. Exit code is non-zero before any drive runs.
2. Stderr names `FT-NOT-REAL` and lists known prefixes.

### Test D — all-Done sweep

Re-configure every feature to the `Done` row; run `dec drive def-ready --all`.

Assertions:

1. Exit code is `0`.
2. Tally `done == len(features)`, every other bucket `0`.

### Boundary

- Two invocations against an unchanged store produce byte-identical JSON
  output modulo the `elapsed_ms` field per row. The fixture freezes the
  inspector clock so elapsed values are also deterministic in tests.

## Notes

This TC also pins the "lift sweep into core::drive::sweep" claim from
FT-119's spec: a regression that re-coupled the sweep to FT-111's specific
goal would fail this test because the sweep would not accept `Goal::DefReady`.
