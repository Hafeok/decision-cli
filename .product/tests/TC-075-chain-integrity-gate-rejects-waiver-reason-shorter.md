---
id: TC-075
title: chain-integrity gate rejects waiver reason shorter than minimum length
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: cargo-test
runner-args: tc_075_chain_integrity_gate_rejects_waiver_reason_shorter
runner-timeout: 120
last-run: 2026-05-24T19:14:07.082851822+00:00
last-run-duration: 0.4s
---

## Premise

The caller invokes `dec implement FT-U --waive-coverage "too short"` (length < 16) — or `--waive-coverage "                "` (whitespace-only).

## Acceptance Criteria

- The dispatch fails with `Error::InvalidArgument { field: "waiver.reason" }`.
- Exit code is 2.
- No `CoverageWaiver` artifact is written.
- The implementer is not invoked.
- The error message names the minimum length and rejects whitespace-only input.

## Notes

The waiver mechanism's value depends on the reason being meaningful. A short or empty reason defeats the audit trail that makes [ADR-031](ADR-031)'s escape hatch tolerable.