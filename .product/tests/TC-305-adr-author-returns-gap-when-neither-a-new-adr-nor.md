---
id: TC-305
title: adr-author returns gap when neither a new ADR nor a reasoned acknowledgement is defensible
type: scenario
status: unimplemented
validates:
  features:
  - FT-130
  adrs:
  - ADR-073
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/adr-author/tests/test_gap_undefensible.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Validates FT-130 (adr-author worker). When a preflight brief is too under-specified to support either a defensible net-new ADR (insufficient context for Context/Decision/Consequences) or a reasoned acknowledgement (no existing ADR plausibly fits), adr-author emits `kind: "gap"` with `missing_information` so the planner routes upstream for human enrichment rather than fabricating a decision.

## Acceptance

- Parsed stdout `AdrProposal.kind` equals `"gap"`.
- The proposal's `gap.missing_information` array has length at least 1.
- The proposal's `gap.reason` string is non-empty.
- Neither the `new` payload nor the `acknowledgement` payload is populated.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: a preflight gap with no scope, no problem statement, no existing ADR candidates — essentially a placeholder title alone. The Anthropic client is monkeypatched to return an `AdrProposal(kind="gap", gap=Gap(missing_information=["scope", "problem statement", "candidate ADRs"], reason=<explanation>))`.

## Out of scope

- New-ADR (TC-302) and acknowledgement (TC-303) paths.
- Bare-ack defence (TC-304).

