---
id: TC-320
title: adr-quality emits approved for a new ADR that schema-conforms and soundly closes the preflight gap
type: scenario
status: passing
validates:
  features:
  - FT-133
  adrs:
  - ADR-073
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/adr-quality/tests/test_approved_new.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T18:41:43.104580599+00:00
last-run-duration: 0.6s
---

## Purpose

Exercise the FT-133 adr-quality judge worker on the happy path for a `new`-kind AdrProposal. The proposal closes a preflight gap with a fresh ADR that has every required H2 section, the right scope for the gap, and at least two substantive Rejected alternatives. This TC validates the five-criterion new-ADR rubric of ADR-073 and the QualityVerdict shape of ADR-074.

## Acceptance

- Worker exits with code 0.
- Emitted verdict has `verdict == "approved"`.
- `rationale` walks the five new-ADR rubric criteria (schema-conforming, gap-closing, scope-correct, alternatives-noted, traceable) and notes each as satisfied.
- `violates` is empty.
- `bundle_hash` in the verdict echoes the hash of the input bundle.

## Inputs

A synthetic bundle containing one `dec:AdrProposal` with `kind: new`, whose markdown body has every required ADR H2 (Status, Context, Decision, Consequences, Rejected alternatives), a `scope` value matching the `dec:PreflightGap` kind in the bundle, and a Rejected alternatives section listing two or more substantive alternatives. The mocked Claude response returns a `QualityVerdict` payload with `verdict: "approved"` and a rationale enumerating the five rubric criteria.

## Out of scope

- Does not exercise the acknowledgement-kind path (TC-321 covers that).
- Does not exercise rejection on scope mismatch or bare alternatives (TC-322 covers that).