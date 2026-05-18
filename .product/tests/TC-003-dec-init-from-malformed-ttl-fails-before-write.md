---
id: TC-003
title: dec_init_from_malformed_ttl_fails_before_write
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-003-malformed-ttl.sh
runner-timeout: 30
last-run: 2026-05-18T19:02:15.368687079+00:00
last-run-duration: 0.2s
---

## Purpose

Validates the **ADR-006** "no state written unless all five validation steps pass" guarantee for the parse / SHACL stages: a malformed or schema-violating definition must fail **before** writing any state, with a clear SHACL violation message naming the missing or invalid fields (FT-008).

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #3.

## Given

- A fresh working directory with no `.dec/`.
- A `bad.ttl` that parses as Turtle but **omits** one or more SHACL-required fields (e.g., missing `dec:title` or `dec:authorizedGoals`).

## When

```bash
dec init --from ./bad.ttl
```

## Then

1. The command exits non-zero.
2. stderr names at least one violated SHACL shape and the missing/invalid field (e.g., "`dec:ValueStream` requires `dec:authorizedGoals` (sh:minCount 1)").
3. **No `.dec/` directory is created.** The working directory is in the same state as before the command.
4. Re-running `dec init` with a valid input after the failure succeeds with no residual corruption (additional check; not strictly required by the source bullet but proves the no-state-written claim).

## Notes

- This is the structural integrity check that makes ADR-006 trustworthy.
- TC-004 and TC-005 cover the URI-resolution and goal-cross-validation branches.