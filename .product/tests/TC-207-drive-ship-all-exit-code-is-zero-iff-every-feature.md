---
id: TC-207
title: Drive ship --all exit code is zero iff every feature outcome is Done
type: scenario
status: passing
validates:
  features:
  - FT-111
  adrs: []
observes:
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-207-drive-ship-all-exit-code.sh
runner-timeout: 120
last-run: 2026-05-29T09:26:03.704087369+00:00
last-run-duration: 0.0s
---

## Description

The exit code is the gate operators chain into other commands
(`dec drive ship --all && deploy`). Per FT-111 §Behaviour step 7,
0 means every feature reached `Done`; any non-Done outcome
yields non-zero.

## Acceptance Criteria

Bash test that composes a `.dec` + `.product` temp workspace and
runs the CLI directly via the installed `dec` binary.

**Case 1 — all-done is exit 0:**
Seed two features (`FT-A`, `FT-B`), each with an approved VGR
already in place so the planner's table returns `Done`
immediately. Run `dec drive ship --all`. Assert exit code is 0.

**Case 2 — any non-done is exit 1:**
Seed two features (`FT-A` approved, `FT-B` with no covering
graphs and `--max-iter 1` so the planner returns stuck quickly).
Run `dec drive ship --all`. Assert exit code is 1. The TSV /
text output should still appear (failure does not suppress the
report).

The bash script writes the seed Turtle, runs the binary, and
exits with PASS only when both cases match.