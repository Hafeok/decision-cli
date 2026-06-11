---
id: TC-430
title: Audit positive path — a correct cell set passes all five checks including the worktree compile probe
type: exit-criteria
status: passing
validates:
  features:
  - FT-172
  adrs:
  - ADR-080
  - ADR-013
phase: 1
runner: bash
runner-args: scripts/checks/tc-430-audit-positive-five-checks.sh
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T18:18:31.674452582+00:00
last-run-duration: 1.6s
---

## Purpose

FT-172 positive path: a correct cell set — the operator-promoted FT-147 archetype files from the live tree — passes all five checks, including the git-worktree compile probe (`cargo check -p dec-ontology --all-targets` against HEAD with auto-wired module declarations).

## Mechanism

`scripts/checks/tc-430-audit-positive-five-checks.sh` builds a fixture from the live tree and asserts `PASS add-artifact-type (5 checks passed)`.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code 1 — a correct cell set failed a hardened check (false positive).