---
id: TC-262
title: FT-120 dec _retract-orphan-defects --graph VG-NNN --dry-run lists candidates without writing
type: scenario
status: unimplemented
validates:
  features:
  - FT-120
  adrs:
  - ADR-024
phase: 4
runner: bash
runner-args: tests/scripts/tc-262-orphan-retract-dry-run.sh
runner-timeout: 60
---

## Description

The hidden diagnostic CLI lists candidate orphan defects for a
specific graph and does not mutate the store when `--dry-run` is
set.

## Acceptance criteria

1. **Dry-run lists candidates.** `dec _retract-orphan-defects
   --graph VG-001 --dry-run` against a fixture store with N
   orphaned defects from VG-001 prints exactly N feedback IRIs (one
   per line) with the source TC IRI alongside.
2. **No writes on dry-run.** After the dry-run, the lifecycle
   state of every listed feedback is unchanged in the store.
3. **Live invocation writes.** Same fixture, second invocation
   without `--dry-run`, mutates each listed feedback to
   `superseded`.
4. **Idempotency.** Re-running the live invocation finds zero
   candidates and exits 0.
5. **Non-existent graph.** `--graph VG-DOES-NOT-EXIST` exits 2 with
   a clear error message.
6. **Hidden from help.** The command is not listed in `dec --help`
   (matches FT-116's `_retract-stale-defects` convention).

## Runner

`bash` script: `tests/scripts/tc-262-orphan-retract-dry-run.sh`.
