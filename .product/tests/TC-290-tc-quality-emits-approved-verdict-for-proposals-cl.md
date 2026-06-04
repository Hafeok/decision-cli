---
id: TC-290
title: tc-quality emits approved verdict for proposals clearing every rubric criterion
type: scenario
status: passing
validates:
  features:
  - FT-127
  adrs:
  - ADR-073
  - ADR-074
  - ADR-075
phase: 1
runner: pytest
runner-args: workers/tc-quality/tests/test_approved.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T11:59:46.203570490+00:00
last-run-duration: 0.3s
---

## Purpose

Validates FT-127 (tc-quality worker). When a `TcProposal` is fed to tc-quality whose every `ProposedTc` clears all five rubric criteria, the worker must emit a `QualityVerdict` of kind `approved` carrying a substantive rationale, no violates entries, no amendment guidance, and the required `judges`/`against` provenance fields per ADR-074. This is the green-path that unblocks ADR-075 auto-accept for tc-quality.

## Acceptance

- Parsed stdout deserialises to a `QualityVerdict` whose `verdict` equals `"approved"`.
- The verdict's `rationale` length is at least 20 characters.
- The verdict's `violates` array is empty.
- The verdict's `amendment_guidance` field is `None` / null.
- The verdict's `judges` IRI and `against` IRI list are both populated (non-empty), and the worker echoes the input `bundle_hash`.

## Inputs

Synthetic bundle JSON: a `TcProposal` whose `new.tcs` array contains entries that conform to the rubric (non-redundant, well-scoped, wireable runners, populated observes, substantive description). The Anthropic client is monkeypatched to return a canned `QualityVerdict(verdict="approved", rationale="...", judges=..., against=[...])`. The test invokes `python -m tc_quality <bundle-path>`.

## Out of scope

- Rejection or amendment behaviour (covered by TC-291 / TC-292).
- Provenance shape beyond presence/non-emptiness (covered by TC-293).