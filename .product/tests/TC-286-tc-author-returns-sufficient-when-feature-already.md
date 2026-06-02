---
id: TC-286
title: tc-author returns sufficient when feature already meets min_tcs_per_feature
type: scenario
status: unimplemented
validates:
  features:
  - FT-126
  adrs:
  - ADR-073
  - ADR-074
  - ADR-072
phase: 1
runner: pytest
runner-args: workers/tc-author/tests/test_sufficient.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Validates FT-126 (tc-author worker). When the feature already meets the per-feature TC floor and the existing TCs cover every required axis from ADR-072's coverage taxonomy, tc-author must short-circuit and emit `kind: "sufficient"` rather than fabricate redundant TCs. The test asserts that tc-author, given a bundle containing `existing_tcs.len() >= target_count` and a fully-covered axis map, prints a `TcProposal` of kind `sufficient` carrying a non-empty `coverage_map`, per the worker-CLI envelope of ADR-073 and the proposal/verdict split of ADR-074.

## Acceptance

- Parsed stdout deserialises to a `TcProposal` whose `kind` field equals `"sufficient"`.
- The proposal's `coverage_map` is present and non-empty (at least one axis-to-existing-TC mapping).
- Neither the `new` payload nor the `augment` payload is populated.
- The worker exits with status code 0.
- The proposal's `bundle_hash` field equals the `bundle_hash` of the synthetic input bundle (per ADR-073 envelope echo requirement).

## Inputs

Synthetic bundle JSON written to a temp dir: feature_spec FT-stub with `min_tcs_per_feature = 3`, three existing TCs whose `observes` collectively cover every axis in the coverage taxonomy fixture, and an `existing_tcs` array of length 3. The Anthropic client is monkeypatched at the worker boundary to return a canned `TcProposal(kind="sufficient", coverage_map={...})`; the test then invokes `python -m tc_author` with the bundle path and parses stdout.

## Out of scope

- Behaviour when `existing_tcs.len() < target_count` (covered by TC-287).
- Validation of `runner_args` shape against the runner vocabulary (covered by TC-288).

