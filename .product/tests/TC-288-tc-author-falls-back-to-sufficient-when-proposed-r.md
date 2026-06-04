---
id: TC-288
title: tc-author falls back to sufficient when proposed runners fail args_pattern validation
type: scenario
status: passing
validates:
  features:
  - FT-126
  adrs:
  - ADR-073
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/tc-author/tests/test_runner_args_fallback.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T09:19:14.427479625+00:00
last-run-duration: 0.3s
---

## Purpose

Validates FT-126 (tc-author worker). Exercises the retry-budget-1 boundary defined in ADR-073: when Claude's first reply contains `new` TCs whose `runner_args` fail the runner-vocabulary `args_pattern`, tc-author retries once; if the retry also fails, the worker MUST fall back to `kind: "sufficient"` with a reasoning string that names the failure rather than emit invalid TCs. This protects the harness from ever ingesting non-wireable runner pairs.

## Acceptance

- Parsed stdout deserialises to a `TcProposal` whose `kind` equals `"sufficient"`.
- The proposal's `reasoning` string contains the substring `"could not produce wireable"` (or equivalent failure-naming language).
- Neither the `new` payload nor the `augment` payload is populated.
- The worker exits with status code 0 (graceful fallback, not error).
- The mocked Anthropic client recorded exactly two calls (initial attempt + one retry), confirming the retry-budget-1 contract.

## Inputs

Synthetic bundle JSON: a feature with `min_tcs_per_feature = 3` and `existing_tcs: []`. The Anthropic client is monkeypatched to return, on both call 1 and call 2, a `TcProposal(kind="new", new=ProposedNew(tcs=[...]))` where every `tc.runner_args` is malformed (e.g. `runner: cargo-test, runner_args: ""` or shell metacharacters). The test counts `client.messages.create` invocations.

## Out of scope

- Success-path retry where the second attempt yields valid TCs (the budget-1 fallback is the focus).
- Verdict adjudication of the fallback `sufficient` proposal (covered by FT-127 TCs).