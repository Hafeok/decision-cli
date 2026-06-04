---
id: TC-303
title: adr-author returns acknowledgement with reasoning >= 40 chars for an existing-ADR gap
type: scenario
status: passing
validates:
  features:
  - FT-130
  adrs:
  - ADR-073
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/adr-author/tests/test_acknowledgement.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T18:41:39.023702542+00:00
last-run-duration: 0.4s
---

## Purpose

Validates FT-130 (adr-author worker). When a preflight gap is in fact governed by an existing ADR that was simply not linked, adr-author returns `kind: "acknowledgement"` with substantive `reasoning` (>= 40 chars) explaining the connection and an `acknowledges` field naming the existing ADR id. This avoids creating duplicate ADRs and preserves graph minimality.

## Acceptance

- Parsed stdout `AdrProposal.kind` equals `"acknowledgement"`.
- The proposal's `acknowledgement.reasoning` string length is at least 40 chars.
- The proposal's `acknowledgement.acknowledges` field references an existing ADR id present in the input bundle (e.g. `"ADR-014"`).
- The proposal echoes the input `bundle_hash`.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: a preflight gap whose subject is in fact governed by ADR-EX (present in the bundle's `existing_adrs` list) but the gap exists because the feature_spec wasn't linked to ADR-EX. The Anthropic client is monkeypatched to return an `AdrProposal(kind="acknowledgement", acknowledgement=Acknowledgement(acknowledges="ADR-EX", reasoning=<substantive explanation>))`.

## Out of scope

- New-ADR path (covered by TC-302).
- Bare-ack defence (covered by TC-304).
- Gap path (covered by TC-305).