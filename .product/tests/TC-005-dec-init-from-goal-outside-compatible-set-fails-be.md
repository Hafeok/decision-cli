---
id: TC-005
title: dec_init_from_goal_outside_compatible_set_fails_before_write
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-005-goal-outside-compatible-set.sh
runner-timeout: 30
last-run: 2026-05-18T19:02:15.368687079+00:00
last-run-duration: 0.2s
---

## Purpose

Validates the cross-validate step of the **ADR-006** pipeline: a definition whose `dec:authorizedGoals` includes a verb **not** in the referenced ValueAction's compatible-goals must fail **before** writing state, naming both the goal and the compatible set.

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #5.

## Given

- A fresh working directory with no `.dec/`.
- A `bad-goal.ttl` referencing `va:shipped-feature` but declaring `dec:authorizedGoals ( "prioritize" )` — where `prioritize` is NOT in `va:shipped-feature`'s compatible-goals (which per §3.2 example includes only verbs aligned to shipping, like `ship`, `land`).

## When

```bash
dec init --from ./bad-goal.ttl
```

## Then

1. The command exits non-zero.
2. stderr names the unauthorized goal (`prioritize`) **and** the ValueAction's compatible-goals set (e.g., `ship, land, …`), as well as the referenced ValueAction URI.
3. **No `.dec/` directory is created.**

## Notes

- The error message phrasing should match §3.4: "This stream pursues `va:shipped-feature`; `prioritize` is not an authorized goal — try a stream with Discovery scope."
- TC-007 validates the runtime variant (a properly-initialized store refuses an unauthorized goal at command time).