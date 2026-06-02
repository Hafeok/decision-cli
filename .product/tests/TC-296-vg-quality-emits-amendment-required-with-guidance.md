---
id: TC-296
title: vg-quality emits amendment-required with guidance for missing or over-claimed evidence mappings
type: scenario
status: unimplemented
validates:
  features:
  - FT-128
  adrs:
  - ADR-073
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/vg-quality/tests/test_amendment_required.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Validates FT-128 (vg-quality worker). When steps under- or over-claim `provides_evidence_for` — e.g. a step claims to cover TC-X but its op produces no signal for TC-X, or omits a step that obviously covers TC-Y — vg-quality emits `verdict: "amendment-required"` with concrete guidance pointing the author at the misaligned mapping. This keeps the VG author cycle short.

## Acceptance

- Parsed stdout `QualityVerdict.verdict` equals `"amendment-required"`.
- The verdict's `amendment_guidance` is a string of length at least 20 naming the over/under-claiming step.
- The verdict's `violates` array contains the IRI / id of the misaligned step.
- The verdict's `against` array references the env and at least one covered_tc.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: a `GraphProposal(kind="new")` whose steps claim `provides_evidence_for=[TC-Z]` but use an op the rubric knows cannot satisfy TC-Z, OR omit any step covering an entry in `covered_tcs`. The Anthropic client is monkeypatched to return a canned amendment-required verdict naming the issue.

## Out of scope

- Approved (TC-294), rejected (TC-295), gap (TC-297) verdicts.
- Re-author cycle iteration counting (covered by FT-131 cycle TCs).

