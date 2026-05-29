---
id: TC-215
title: Reader groups orchestration store events into chronological per-round records
type: scenario
status: unimplemented
validates:
  features:
  - FT-113
  adrs: []
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_215_reader_groups_into_chronological_rounds
runner-timeout: 60
---

## Description

The reader's job is to translate the raw orchestration-store
event soup into a clean `Vec<Round>` ordered by dispatch
timestamp. This is the load-bearing read primitive that the
text renderer and the JSON format both consume.

## Acceptance Criteria

Cargo test:

1. Build a temp orchestration store with three dispatches for
   FT-X (each one a Session activity with a different
   `dec:targetFeature`-resolving role + timestamp):
   - t=0  verify-graph-author session (produces VG-100)
   - t=180  implementer session (produces commit-change-001)
   - t=360  verifier session (produces VGR-500)
2. Call `reader.rounds_for_feature("FT-X", None)`.
3. Assert the returned `Vec<Round>` has length 3.
4. Assert rounds are ordered by `started_at` ascending —
   Round 0 dispatch role is VGA, Round 1 is implementer,
   Round 2 is verifier.
5. Assert each `RoundState` reflects the planner-observable
   snapshot the round would have seen at dispatch time
   (cardinalities only; the reconstruction is best-effort
   per FT-113 §Error handling).

The test stubs the store at the SPARQL boundary — no live
Oxigraph needed beyond the test fixture.
