---
id: TC-304
title: adr-author rejects bare acknowledgements at the worker boundary before stdout
type: scenario
status: passing
validates:
  features:
  - FT-130
  adrs:
  - ADR-073
phase: 1
runner: pytest
runner-args: workers/adr-author/tests/test_bare_ack_rejected.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T18:41:39.023702542+00:00
last-run-duration: 0.4s
---

## Purpose

Validates FT-130 (adr-author worker) against ADR-073's worker-output discipline. A bare acknowledgement (empty or whitespace-only `reasoning`) provides no audit value and would silently consume a planner cycle. The worker MUST refuse to print such a proposal — either by exiting non-zero with a structured error before any stdout, or by falling back to `kind: "gap"`. It must NEVER emit a `kind: "acknowledgement"` whose reasoning is empty.

## Acceptance

- When the mocked Claude returns `AdrProposal(kind="acknowledgement", acknowledgement=Acknowledgement(acknowledges="ADR-X", reasoning=""))`, the actual stdout EITHER (a) parses to `kind: "gap"` OR (b) is empty/error and exit code is non-zero.
- Under no circumstances does parsed stdout deserialise to a `kind: "acknowledgement"` proposal with an empty/whitespace `reasoning`.
- When the exit code is non-zero, stderr contains a structured error message identifying the bare-ack rejection.
- The test runs both the empty-string and whitespace-only variants and both pass the above invariants.

## Inputs

Synthetic bundle JSON: a preflight gap with an existing ADR present. The Anthropic client is monkeypatched to return, in two parameterised cases, `Acknowledgement(reasoning="")` and `Acknowledgement(reasoning="   \n\t ")`. The test captures stdout, stderr, and exit code.

## Out of scope

- The fallback policy decision (gap vs error) — this TC asserts the invariant only, not the chosen branch.
- Verdict adjudication of fallback gaps (covered by ADR/spec-quality worker TCs if added later).