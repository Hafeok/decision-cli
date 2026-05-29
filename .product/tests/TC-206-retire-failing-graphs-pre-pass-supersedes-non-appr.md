---
id: TC-206
title: Retire-failing-graphs pre-pass supersedes non-approved covering graphs only when flag set
type: scenario
status: passing
validates:
  features:
  - FT-111
  adrs: []
observes:
- file
- stdout
phase: 4
runner: cargo-test
runner-args: tc_206_retire_failing_graphs_pre_pass
runner-timeout: 60
last-run: 2026-05-29T09:31:56.032729503+00:00
last-run-duration: 0.5s
---

## Description

PAT-003 anti-pattern: "pre-pass that mutates state without a
flag." The retire-failing-graphs pre-pass exists for the bash
script's "wipe-and-rerun" workflow but must be opt-in in the
CLI — silently mutating the orchestration store on every
invocation is a hard-to-debug surprise.

## Acceptance Criteria

Compose a temp orchestration store containing:

- VG-100: covers FT-X, latest VGR is `approved`. (Should not be
  retired.)
- VG-101: covers FT-X, latest VGR is `rejected`. (Candidate for
  retirement.)
- VG-102: covers FT-X, no VGR. (Should not be retired — no
  evidence either way.)

**Case 1 — flag off:**
Run the sweep with `retire_failing_graphs = false`. Assert no
`dec:supersededBy` edge added to any of the three graphs.

**Case 2 — flag on:**
Run the sweep with `retire_failing_graphs = true`. Assert:

1. VG-101 has a fresh `dec:supersededBy <urn:dec:retired-stale-sweep-...>` edge.
2. VG-100 and VG-102 are untouched.
3. The sweep's detail log records "retired 1 stale graphs" for
   FT-X.

Inspect the orchestration store via SPARQL after the run; do not
trust the sweep's return value alone.