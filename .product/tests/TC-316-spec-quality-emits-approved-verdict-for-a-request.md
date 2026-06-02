---
id: TC-316
title: spec-quality emits approved verdict for a request-faithful schema-conforming SpecProposal
type: scenario
status: unimplemented
validates:
  features:
  - FT-132
  adrs:
  - ADR-073
  - ADR-074
  - ADR-075
  - ADR-047
phase: 1
runner: pytest
runner-args: workers/spec-quality/tests/test_approved.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Exercise the FT-132 spec-quality judge worker on the happy path. The worker reads a SpecProposal bundle (emitted upstream by FT-129 spec-author) together with the originating request and emits a `dec:QualityVerdict` whose verdict is `approved`. This TC validates the schema-conforming rubric of ADR-073, the QualityVerdict shape of ADR-074, the rubric-flips contract of ADR-075, and the feature_spec section schema of ADR-047.

## Acceptance

- Worker exits with code 0.
- Emitted verdict JSON has `verdict == "approved"`.
- `rationale` field is a non-empty string of length >= 20 characters.
- `violates` is an empty list and `amendment_guidance` is absent or empty.
- `judges` resolves to the SpecProposal IRI in the bundle; `against` contains exactly the originating request IRI.
- `bundle_hash` in the verdict echoes the hash of the input bundle byte-for-byte.

## Inputs

A synthetic bundle containing one `dec:SpecProposal` IRI whose markdown body has every required H2 (Purpose, Functional Specification, Out of scope, Open questions) plus every required Functional Specification H3 subsection per ADR-047/FT-055, and whose behaviour assertions textually trace to a single `dec:Request` IRI also present in the bundle. The mocked Claude response returns a structured `QualityVerdict` payload with `verdict: "approved"` and a substantive rationale.

## Out of scope

- Does not assert the worker's behaviour when the SpecProposal IRI cannot be resolved in the bundle (covered by separate substrate tests).
- Does not validate the orchestrator's persistence of the verdict to the named graph (that is harness-side, covered under FT-127/FT-128 parity tests).

