---
id: TC-025
title: value_stream_scope_chokepoint_in_place
type: invariant
status: failing
validates:
  features: []
  adrs:
  - ADR-005
phase: 1
runner: bash
runner-args: scripts/checks/value-stream-scope.sh
runner-timeout: 60
last-run: 2026-05-20T08:26:41.315265110+00:00
failure-message: "ERROR: expected crates/decision-cli/src/scope (ADR-005 anchor)\n"
last-run-duration: 0.0s
---

## Purpose

Mechanical enforcement of **ADR-005 ValueStream as graph-resident scope**.
Asserts the scope module surfaces the §3.4 chokepoint: `ActiveScope::load`
(loads the persisted ValueStream), `validate_goal` (enforces the
authorized-goals set), and the `UnauthorizedGoal` error variant
(structured refusal seen by operators).

TC-007 exercises this chokepoint end-to-end; this TC ensures the
chokepoint *exists* at all so a refactor cannot silently remove it.

## Given

- A working copy of decision-cli with `crates/decision-cli/src/scope/`
  present.
- `bash` and `grep` available on `PATH`.

## When

```bash
scripts/checks/value-stream-scope.sh
```

## Then

1. Exit 0 if `ActiveScope::load`, `validate_goal`, and `UnauthorizedGoal`
   appear in the scope module.
2. Exit 1 if any of the three is missing (the §3.4 chokepoint regressed).