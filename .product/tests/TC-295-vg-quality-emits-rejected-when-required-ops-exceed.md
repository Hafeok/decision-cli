---
id: TC-295
title: vg-quality emits rejected when required_ops exceeds env allowed_ops or graph is non-minimal
type: scenario
status: unimplemented
validates:
  features:
  - FT-128
  adrs:
  - ADR-073
  - ADR-074
  - ADR-030
phase: 1
runner: pytest
runner-args: workers/vg-quality/tests/test_rejected.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Validates FT-128 (vg-quality worker). When the union of step `required_ops` contains an op not in `env.allowed_ops`, OR when two steps are functionally redundant (same evidence target, same op signature), vg-quality must emit `verdict: "rejected"` with `violates` naming the offending step or env id and a rationale citing the failed rubric criterion. This guards ADR-030's env-isolation and minimality invariants.

## Acceptance

- Parsed stdout `QualityVerdict.verdict` equals `"rejected"`.
- The verdict's `violates` array contains the IRI / id of the offending step OR the env id (depending on the failure mode).
- The verdict's `rationale` names the failed criterion (`"env-bound"` or `"minimal"` / `"non-redundant"`).
- The verdict's `amendment_guidance` is `None`.
- The worker exits with status code 0.

## Inputs

Two parameterised pytest cases sharing the same bundle skeleton: (a) `Step(required_ops=[forbidden_op])` paired with `env.allowed_ops=[op1, op2]` excluding `forbidden_op`; (b) two `Step` entries with identical `required_ops` and `provides_evidence_for`. The Anthropic client is monkeypatched to return a canned rejected verdict per case naming the offending element.

## Out of scope

- Approved (TC-294), amendment (TC-296), gap (TC-297) paths.
- The harness's downstream re-author cycle.

