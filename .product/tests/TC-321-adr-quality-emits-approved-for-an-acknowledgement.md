---
id: TC-321
title: adr-quality emits approved for an acknowledgement with reasoning >= 40 chars referencing an existing ADR
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
runner-args: workers/adr-quality/tests/test_approved_ack.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T18:41:43.104580599+00:00
last-run-duration: 0.6s
---

## Purpose

Exercise the FT-133 adr-quality judge worker on the acknowledgement-kind happy path. When the preflight gap is closeable by acknowledging an existing ADR rather than authoring a new one, the AdrProposal carries `kind: acknowledgement` plus a `reasoning` field. The worker must approve when the reasoning is substantive (>= 40 chars) and the referenced ADR genuinely governs the feature. This TC validates the acknowledgement rubric of ADR-073 and the QualityVerdict shape of ADR-074.

## Acceptance

- Worker exits with code 0.
- Emitted verdict has `verdict == "approved"`.
- `rationale` cites the acknowledgement rubric criteria (reasoning-substantive, governs-feature).
- `against` contains both the `dec:PreflightGap` IRI and the `dec:FeatureSpec` IRI from the bundle.
- `violates` is empty.

## Inputs

A synthetic bundle containing one `dec:AdrProposal` with `kind: acknowledgement`, a `reasoning` string of >= 40 characters, and a reference to an existing `dec:Adr` IRI that is itself linked (in the bundle) to the `dec:FeatureSpec` under review. The bundle also carries a `dec:PreflightGap` IRI. The mocked Claude response returns a `QualityVerdict` payload with `verdict: "approved"` whose `against` lists both the gap and the feature_spec.

## Out of scope

- Does not assert the worker's behaviour when the referenced ADR is missing or does not govern the feature (negative-path coverage is folded into TC-322's broader rejection scenarios).
- Does not exercise the new-kind path (TC-320 covers that).