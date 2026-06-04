---
id: TC-287
title: tc-author returns new with target_count TCs and wired runner fields for an under-covered feature
type: scenario
status: passing
validates:
  features:
  - FT-126
  adrs:
  - ADR-073
  - ADR-074
  - ADR-072
phase: 1
runner: pytest
runner-args: workers/tc-author/tests/test_new_with_runners.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T09:19:14.427479625+00:00
last-run-duration: 0.4s
---

## Purpose

Validates FT-126 (tc-author worker). When the feature has zero existing TCs and the target count is four, tc-author must emit exactly four well-formed `ProposedTc` entries, each carrying a wireable `runner` + `runner_args` pair drawn from the runner vocabulary (ADR-072). This is the primary positive path the planner depends on for the FT-131 author cycle and exercises the proposal envelope of ADR-073 / ADR-074.

## Acceptance

- Parsed stdout deserialises to a `TcProposal` whose `kind` equals `"new"`.
- `new.tcs.len()` equals exactly 4 (the `target_count` supplied in the bundle).
- Every `tc.runner_args` is a non-empty string.
- Every `tc.runner` is in the allowed runner enum (`cargo-test`, `bash`, `pytest`, etc., per the runner vocabulary fixture).
- Every `tc.observes` array is non-empty (at least one axis label).
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: feature_spec FT-stub with `min_tcs_per_feature = 4`, `existing_tcs: []`, and a runner-vocabulary fixture from ADR-072. The Anthropic client is monkeypatched to return a canned `TcProposal(kind="new", new=ProposedNew(tcs=[...4 well-formed entries...]))`. The test invokes `python -m tc_author <bundle-path>` and parses stdout JSON.

## Out of scope

- Verdict adjudication of the proposed TCs (covered by FT-127 TCs).
- Retry / fallback behaviour when proposals fail vocabulary validation (covered by TC-288).