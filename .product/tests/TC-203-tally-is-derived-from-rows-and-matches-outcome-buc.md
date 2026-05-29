---
id: TC-203
title: Tally is derived from rows and matches outcome bucket counts
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
runner-args: tc_203_tally_matches_row_bucket_counts
runner-timeout: 30
---

## Description

PAT-003 invariant: tally is computed FROM the rows after the
loop, not tracked in parallel during the loop. This catches the
class of bug where row-construction silently drops a feature
(panic at boundary, early-break) but the running tally counter
already incremented — tally and rows would silently disagree.

## Acceptance Criteria

Construct a `Vec<SweepRow>` by hand with a known mix of
outcomes:

- 3 × `Done`
- 2 × `Stuck { reason: ... }`
- 1 × `HitMaxIter`
- 1 × `Timeout { after_secs: 600 }`
- 1 × `Error { detail: ... }`

Call `derive_tally(&rows)` (or whatever the function is named).
Assert the returned `SweepTally` has exactly:
`done=3, stuck=2, max_iter=1, timeout=1, error=1`.

Pure function test, no async runtime needed.
