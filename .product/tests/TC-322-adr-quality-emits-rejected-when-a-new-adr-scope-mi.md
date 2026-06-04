---
id: TC-322
title: adr-quality emits rejected when a new ADR scope mismatches the gap kind or alternatives section is bare
type: scenario
status: passing
validates:
  features:
  - FT-133
  adrs:
  - ADR-073
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/adr-quality/tests/test_rejected_scope_mismatch.py
runner-timeout: 60
observes:
- exit-code
- stdout
last-run: 2026-06-04T18:41:43.104580599+00:00
last-run-duration: 0.4s
---

## Purpose

Exercise the FT-133 adr-quality judge worker on two rejection paths for `new`-kind AdrProposals: (a) the `scope` value does not match the `preflight_gap` kind (e.g. a cross-cutting gap is proposed as a feature-specific ADR), and (b) the Rejected alternatives section is bare (zero substantive alternatives). Either failure triggers `verdict: "rejected"` with a `violates` entry naming the failing criterion. This TC validates the scope-correct and alternatives-noted criteria of ADR-073 and the QualityVerdict shape of ADR-074.

## Acceptance

- Worker exits with code 0.
- Emitted verdict has `verdict == "rejected"`.
- `violates` is non-empty and names either `scope-correct` or `alternatives-noted` (matching the failure mode in the input).
- `rationale` names the failing rubric criterion explicitly.
- `judges` resolves to the AdrProposal IRI; `bundle_hash` echoes the input.

## Inputs

A synthetic bundle containing one `dec:AdrProposal` with `kind: new`, paired with a `dec:PreflightGap` whose kind is `cross-cutting`. The proposal either declares `scope: feature-specific` (scope mismatch) or has a Rejected alternatives H2 with no body content (bare alternatives). The mocked Claude response returns a `QualityVerdict` with `verdict: "rejected"` and a `violates` entry naming the failing criterion.

## Out of scope

- Does not assert behaviour when both failure modes are present simultaneously (each variant runs independently within the test).
- Does not exercise schema-conforming H2-missing rejections (those are structurally identical to TC-317's analogue on the spec side).