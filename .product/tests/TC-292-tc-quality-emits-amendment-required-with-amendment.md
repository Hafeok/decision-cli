---
id: TC-292
title: tc-quality emits amendment-required with amendment_guidance for fixable style or runner issues
type: scenario
status: passing
validates:
  features:
  - FT-127
  adrs:
  - ADR-073
  - ADR-074
  - ADR-027
phase: 1
runner: pytest
runner-args: workers/tc-quality/tests/test_amendment_required.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T11:59:46.203570490+00:00
last-run-duration: 0.3s
---

## Purpose

Validates FT-127 (tc-quality worker). When the only rubric failures are within ADR-027 mayDecide scope — title style, runner-timeout numeric shape, observes axis labelling — tc-quality must emit `verdict: "amendment-required"` with concrete `amendment_guidance` rather than rejecting outright. This keeps the author cycle short by feeding back actionable edits.

## Acceptance

- Parsed stdout deserialises to a `QualityVerdict` whose `verdict` equals `"amendment-required"`.
- The verdict's `amendment_guidance` is a string of length at least 20.
- The verdict's `violates` array is non-empty (names which proposed TC needs amendment).
- The amendment_guidance text references the mayDecide-class issue (style / naming / timeout) rather than a fundamental rubric violation.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: `TcProposal(kind="new", new=ProposedNew(tcs=[ProposedTc(title="bad title style", runner_timeout="60", ...)]))` — well-formed semantically but tripping a style or numeric-shape rule. The Anthropic client is monkeypatched to return a canned amendment-required verdict naming the fix.

## Out of scope

- Approval (TC-290) and rejection (TC-291) paths.
- The harness's downstream re-author cycle (covered by FT-131 planner TCs).