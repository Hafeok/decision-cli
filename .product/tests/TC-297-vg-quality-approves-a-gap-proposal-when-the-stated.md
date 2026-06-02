---
id: TC-297
title: vg-quality approves a Gap proposal when the stated vocabulary-insufficiency reasoning holds
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
runner-args: workers/vg-quality/tests/test_gap_passthrough.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Validates FT-128 (vg-quality worker). When verify-graph-author returns a `kind: "gap"` GraphProposal because no combination of `step_vocabulary` ops can demonstrate one of the `covered_tcs`, vg-quality must validate the gap reasoning and emit `verdict: "approved"` so the harness can route the gap upstream as feedback rather than treating it as a malformed proposal.

## Acceptance

- Parsed stdout `QualityVerdict.verdict` equals `"approved"`.
- The verdict's `rationale` validates the gap reason (confirms the vocabulary truly cannot cover the listed TC, given the env's allowed_ops).
- The verdict's `against` array includes the covered_tc IRI that triggered the gap and the env IRI.
- The verdict's `violates` array is empty.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: a `GraphProposal(kind="gap", gap=Gap(reason="op vocabulary does not cover TC-Z which requires multi-host signal", missing_ops=["multi_host_op"]))` paired with an env whose `allowed_ops` and `step_vocabulary` indeed do not include any multi-host capable op. The Anthropic client is monkeypatched to return an approved verdict validating the reasoning.

## Out of scope

- Gap rejection (when the reason is unsound) — that is a TC-295 / amendment variant.
- The harness's upstream gap routing (covered by FT-131 planner TCs).

