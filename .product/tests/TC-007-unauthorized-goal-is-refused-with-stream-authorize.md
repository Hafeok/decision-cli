---
id: TC-007
title: unauthorized_goal_is_refused_with_stream_authorized_list
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-007-unauthorized-goal.sh
runner-timeout: 30
last-run: 2026-05-28T09:28:12.389265890+00:00
last-run-duration: 0.3s
---

## Purpose

Validates **ADR-005** scope enforcement at command time (FT-010): an unauthorized goal verb is refused **before any role dispatches**, with a structured message naming the stream's authorized goals and the referenced ValueAction.

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #7 and §3.4.

## Given

- A working directory initialized as `decision-cli-development` with authorized goals `(ship land)` referencing `va:shipped-feature`.

## When

A command is invoked with an unauthorized goal. Slice 1 does not yet ship `dec drive`, so the operational variant is exercised through the goal-validation entry-point used by `dec implement` and any other dispatch-initiating command. Implementations may either:

- Invoke the goal-validation function directly with `goal="prioritize"`, or
- Use a slice 1 shim / `--debug-goal` flag, if such an escape exists, to drive the same code path.

## Then

1. The dispatch is **refused**: the exit code is non-zero and no Session, Goal, or Dispatch artifact is written to the orchestration store.
2. stderr / structured error contains, in any order:
   - The unauthorized goal (`prioritize`).
   - The stream's authorized goals list (`ship, land`).
   - The referenced ValueAction URI (`va:shipped-feature`).
3. The message matches the shape from §3.4: "This stream pursues `va:shipped-feature`; `prioritize` is not an authorized goal — try a stream with Discovery scope."

## Notes

- The full `dec drive <goal> <artifact>` verb lands in a later slice (ADR-010, ADR-011 / §6.2). Slice 1 exercises the same enforcement gate through whichever code path triggers dispatch.
- TC-014 validates the complementary write-side invariant (`dec:inStream` triple on every Session/Goal/Dispatch/Event).