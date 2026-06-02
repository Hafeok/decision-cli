---
id: TC-319
title: spec-quality verdict carries dec:judges (SpecProposal) and dec:against (request) per ADR-074 polymorphism
type: scenario
status: unimplemented
validates:
  features:
  - FT-132
  adrs:
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/spec-quality/tests/test_judges_against.py
runner-timeout: 60
observes:
- exit-code
- stdout
- graph
---

## Purpose

Pin the polymorphism contract for the spec-author / spec-quality pair under ADR-074. The emitted `dec:QualityVerdict` must carry `dec:judges` resolving to a node of class `dec:SpecProposal` and `dec:against` containing exactly the originating `dec:Request` IRI. This TC validates the QualityVerdict SHACL shape of ADR-074 as it specialises for FT-132.

## Acceptance

- Worker exits with code 0.
- The verdict's `judges` field is exactly one IRI; that IRI resolves in the bundle to a node typed `dec:SpecProposal`.
- The verdict's `against` field is a list of exactly one IRI; that IRI resolves to a node typed `dec:Request`.
- Neither `judges` nor `against` is empty, satisfying the minCount=1 constraints in the ADR-074 SHACL.
- The verdict serialises cleanly through the QualityVerdict pydantic model without missing-field errors.

## Inputs

A synthetic bundle containing exactly one `dec:SpecProposal` IRI and exactly one `dec:Request` IRI, both typed. The SpecProposal body is approval-grade (so the verdict is approved and the focus stays on shape, not rubric). The mocked Claude response returns a well-formed `QualityVerdict` whose `judges` points at the SpecProposal and whose `against` lists the Request.

## Out of scope

- Does not assert rubric-level fields (verdict, rationale, violates) — TC-316 covers approval semantics.
- Does not validate the symmetric contract on the adr-quality side (TC-323 covers that).

