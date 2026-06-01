---
id: TC-263
title: FT-120 dec _retract-orphan-defects --all sweeps every graph
type: scenario
status: unimplemented
validates:
  features:
  - FT-120
  adrs:
  - ADR-024
phase: 4
runner: bash
runner-args: tests/scripts/tc-263-orphan-retract-all.sh
runner-timeout: 60
---

## Description

`--all` iterates over every VG in the store and retracts orphan
defects from each, producing a structured tally on stdout.

## Acceptance criteria

1. **Multi-graph fixture.** Fixture store has 3 graphs (VG-001,
   VG-002, VG-003) with respectively 2, 0, and 5 orphan defects.
2. **--all retracts all 7.** `dec _retract-orphan-defects --all`
   without `--dry-run` retracts all 7 feedbacks across the 3 graphs
   in a single invocation.
3. **Tally on stdout.** The command emits a per-graph summary
   line: `VG-001: 2 retracted`, `VG-002: 0`, `VG-003: 5
   retracted`, plus a final total `total: 7 retracted`.
4. **--feature scope.** `--feature FT-XXX` restricts the sweep to
   graphs linked to FT-XXX. If 2 of the 3 graphs are linked to
   FT-XXX (containing 7 of the orphan defects), only those 7 are
   retracted; the third graph's defects are untouched.
5. **Dry-run + --all.** `--all --dry-run` produces the same tally
   without writing.
6. **Exit codes.** 0 on success, 1 if any write fails partway, 2
   for malformed arguments.

## Runner

`bash` script: `tests/scripts/tc-263-orphan-retract-all.sh`.
