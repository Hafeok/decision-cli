---
id: TC-073
title: chain-integrity gate refuses dispatch when uncovered TCs and no waiver
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: tc_073_chain_integrity_gate_refuses_dispatch_when_uncover
runner-timeout: 120
last-run: 2026-05-23T16:10:06.049905700+00:00
last-run-duration: 0.4s
---

## Premise

Feature `FT-U` references TCs `[T1, T2]`. No graph references `T2`. The caller invokes `dec implement FT-U` with no `--waive-coverage` flag.

## Acceptance Criteria

- The dispatch fails before invoking the implementer worker.
- Exit code is 1.
- stderr contains:
  - `Error::ChainIntegrity` (or its rendered form),
  - the feature id `FT-U`,
  - the uncovered TC `T2`,
  - the remediation hint `dec verify graph generate FT-U --environment ENV-NNN`,
  - the waiver hint `--waive-coverage "<reason>"`.
- No PROV-O activity is opened — the session is never created.
- No `CoverageWaiver` artifact is written.

## Notes

This is the central acceptance test for [ADR-031](ADR-031). The error must be actionable enough that a first-time user can self-remediate without reading the ADR.