---
id: TC-291
title: tc-quality emits rejected with violates citing redundant or unfaithful proposed TCs
type: scenario
status: unimplemented
validates:
  features:
  - FT-127
  adrs:
  - ADR-073
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/tc-quality/tests/test_rejected.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Validates FT-127 (tc-quality worker). When a `TcProposal` contains a `ProposedTc` that is semantically redundant against an existing TC in the bundle, tc-quality must emit `verdict: "rejected"` and populate `violates` with the offending proposed TC ids, citing the "non-redundant" rubric criterion. Rejected verdicts must NOT auto-flip readiness (ADR-075) and force the planner back to author.

## Acceptance

- Parsed stdout deserialises to a `QualityVerdict` whose `verdict` equals `"rejected"`.
- The verdict's `violates` array contains the IRI / id of the redundant `ProposedTc`.
- The verdict's `rationale` string cites the `"non-redundant"` rubric criterion explicitly.
- The verdict's `amendment_guidance` is `None` (rejected, not amendment-required).
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: an `existing_tcs` array containing TC-EX1 with a specific `observes` set, plus a `TcProposal(kind="new", new=ProposedNew(tcs=[ProposedTc(id="ProposedTC-RED", observes=<same as TC-EX1>, description=<near-duplicate of TC-EX1>)]))`. The Anthropic client is monkeypatched to return a canned rejected verdict naming the redundancy.

## Out of scope

- Approval path (covered by TC-290).
- mayDecide-class amendment guidance (covered by TC-292).

