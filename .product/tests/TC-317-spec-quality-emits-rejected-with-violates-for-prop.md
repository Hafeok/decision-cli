---
id: TC-317
title: spec-quality emits rejected with violates for proposals missing required H2/H3 sections
type: scenario
status: unimplemented
validates:
  features:
  - FT-132
  - FT-134
  adrs:
  - ADR-073
  - ADR-074
  - ADR-047
phase: 1
runner: pytest
runner-args: workers/spec-quality/tests/test_rejected_schema.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Exercise the FT-132 spec-quality judge worker on the schema-violation path. When a SpecProposal body is missing a required H2 (e.g. Functional Specification) or a required H3 subsection under Functional Specification (e.g. Invariants), the worker must reject the proposal and enumerate the missing section markers in `violates`. This TC validates the schema-conforming rubric of ADR-073, the QualityVerdict shape of ADR-074, and the spec section schema of ADR-047 (as anchored by FT-055).

## Acceptance

- Worker exits with code 0 (rejection is a successful judgement, not a worker error).
- Emitted verdict has `verdict == "rejected"`.
- `violates` is a non-empty list whose entries name the missing section markers using stable identifiers (e.g. `Functional Specification > Invariants`).
- `rationale` references the schema-conforming rubric criterion by name.
- `judges` resolves to the SpecProposal IRI; `bundle_hash` echoes the input.

## Inputs

A synthetic bundle containing one `dec:SpecProposal` whose markdown body is missing the `## Functional Specification` H2 and/or one of its required H3 subsections per ADR-047. The mocked Claude response returns a `QualityVerdict` payload with `verdict: "rejected"`, a rationale citing the schema-conforming criterion, and a `violates` list naming the missing section.

## Out of scope

- Does not exercise the request-faithfulness rubric independently (TC-316 covers the conjunction).
- Does not assert ordering of multiple entries within `violates` when more than one section is missing.

