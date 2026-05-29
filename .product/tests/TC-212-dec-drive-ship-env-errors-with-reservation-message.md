---
id: TC-212
title: dec drive ship --env errors with reservation message and exits non-zero
type: scenario
status: unimplemented
validates:
  features:
  - FT-112
  adrs: []
observes:
- stderr
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-212-env-flag-reserved.sh
runner-timeout: 30
---

## Description

The `--env` flag must error visibly during the deprecation
window rather than silently meaning `--bench`. Quiet acceptance
of a wrong flag would let operators drift onto a mental model
that breaks when the deployment-target feature ships and
`--env` switches semantics.

## Acceptance Criteria

Bash test that invokes the installed `dec` binary:

1. `dec drive ship FT-X --env ENV-002` — assert exit code is
   non-zero; assert stderr contains the substring
   `"--env is reserved"` or `"use --bench"` (capture exact
   wording from the implementation).
2. `dec drive ship FT-X --bench BNCH-002` — assert exit
   code reaches the planner stage (may stuck or done or
   error for unrelated reasons, but DOES NOT error on flag
   parsing).
3. `dec verify graph generate FT-X --env ENV-002` — same
   reservation error.
4. `dec verify feature FT-X --env ENV-002` — same reservation
   error.

The script seeds a minimal `.dec` workspace so the binary can
reach argument parsing; it doesn't need a fully populated
store.
