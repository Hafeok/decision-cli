---
id: TC-318
title: spec-quality emits amendment-required with guidance for non-empty but thin Out of scope sections
type: scenario
status: passing
validates:
  features:
  - FT-132
  adrs:
  - ADR-073
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/spec-quality/tests/test_amendment_required.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T18:41:41.022525478+00:00
last-run-duration: 0.4s
---

## Purpose

Exercise the FT-132 spec-quality judge worker's middle verdict. A SpecProposal can be structurally schema-conforming yet fail the "Bounded" rubric criterion when the Out of scope section is tautological or otherwise not substantive. In that case the worker must emit `amendment-required` with actionable `amendment_guidance` rather than a hard rejection. This TC validates the bounded-rubric flip of ADR-073 and the three-valued verdict surface of ADR-074.

## Acceptance

- Worker exits with code 0.
- Emitted verdict has `verdict == "amendment-required"`.
- `amendment_guidance` is a non-empty string of length >= 20 characters and names the section to revise.
- `violates` is non-empty and cites the Bounded rubric criterion.
- `rationale` explains why the schema is conformant but the boundedness rubric still fails.

## Inputs

A synthetic bundle containing one `dec:SpecProposal` whose body has every required H2 and H3 per ADR-047 but whose `## Out of scope` section contains only a tautological restatement (e.g. "Not in scope: things not in scope"). The mocked Claude response returns a `QualityVerdict` payload with `verdict: "amendment-required"` and `amendment_guidance` describing how to make the section substantive.

## Out of scope

- Does not exercise the approved or rejected paths (TC-316 and TC-317 cover those).
- Does not assert that the orchestrator routes the amendment back to spec-author for revision (that is harness-side flow).