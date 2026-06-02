---
id: TC-294
title: vg-quality emits approved when graph minimally demonstrates every covered_tc in the env
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
runner-args: workers/vg-quality/tests/test_approved.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Validates FT-128 (vg-quality worker). When a `new`-kind GraphProposal's union of step `required_ops` is a subset of `env.allowed_ops` AND every `covered_tc` has at least one step that `provides_evidence_for` it AND no step is redundant, vg-quality emits `verdict: "approved"`. This is the green path for the VG arm of ADR-030's step-vocabulary governance.

## Acceptance

- Parsed stdout `QualityVerdict.verdict` equals `"approved"`.
- The verdict's `against` array contains every covered TC IRI from the proposal AND the env IRI.
- The verdict's `rationale` length is at least 20 chars and cites the `"minimal"` rubric criterion.
- The verdict's `violates` array is empty and `amendment_guidance` is `None`.
- The worker exits with status code 0 and echoes the input `bundle_hash`.

## Inputs

Synthetic bundle JSON: a `GraphProposal(kind="new", new=ProposedGraph(steps=[Step(required_ops=[op1], provides_evidence_for=[TC-A]), Step(required_ops=[op2], provides_evidence_for=[TC-B])]))` paired with `env.allowed_ops=[op1, op2]` and `covered_tcs=[TC-A, TC-B]`. The Anthropic client is monkeypatched to return a canned approved verdict.

## Out of scope

- Rejection (TC-295), amendment (TC-296), and gap (TC-297) paths.
- Actual graph execution outcomes (vg-quality only judges the proposal shape).

