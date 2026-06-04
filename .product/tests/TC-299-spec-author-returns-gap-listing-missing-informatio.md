---
id: TC-299
title: spec-author returns gap listing missing_information for an under-specified request
type: scenario
status: failing
validates:
  features:
  - FT-129
  adrs:
  - ADR-073
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/spec-author/tests/test_gap_underspec.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T12:08:02.706715821+00:00
last-run-duration: 0.2s
failure-message: "ERROR: file or directory not found: workers/spec-author/tests/test_gap_underspec.py\n\n"
---

## Purpose

Validates FT-129 (spec-author worker). When the input request is under-specified — missing scope, contradictory constraints, or no stated boundary — spec-author must NOT hallucinate a body. Instead it emits `kind: "gap"` with a populated `missing_information` list and a `reason` so the planner can route the gap upstream for human enrichment.

## Acceptance

- Parsed stdout deserialises to a `SpecProposal` whose `kind` equals `"gap"`.
- The proposal's `gap.missing_information` array has length at least 1.
- The proposal's `gap.reason` string is non-empty.
- Neither the `new` payload nor any other proposal payload is populated.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: a `SpecRequest` with only a title and no scope, no acceptance criteria, contradictory constraints in the description (e.g. "must be synchronous" alongside "must scale to 10k concurrent"). The Anthropic client is monkeypatched to return a `SpecProposal(kind="gap", gap=Gap(missing_information=["scope", "boundary", "constraint resolution"], reason="..."))`.

## Out of scope

- Well-formed new path (covered by TC-298).
- ADR-author's gap behaviour (covered by TC-305).