---
id: TC-323
title: adr-quality verdict carries dec:judges (AdrProposal) and dec:against (preflight_gap + feature_spec) per ADR-074
type: scenario
status: passing
validates:
  features:
  - FT-133
  adrs:
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/adr-quality/tests/test_judges_against.py
runner-timeout: 60
observes:
- exit-code
- stdout
- graph
last-run: 2026-06-04T18:41:43.104580599+00:00
last-run-duration: 0.4s
---

## Purpose

Pin the polymorphism contract for the adr-author / adr-quality pair under ADR-074. The emitted `dec:QualityVerdict` must carry `dec:judges` resolving to a node of class `dec:AdrProposal` and `dec:against` containing exactly the `dec:PreflightGap` IRI plus the `dec:FeatureSpec` IRI that the proposal is closing. This TC validates the QualityVerdict SHACL shape of ADR-074 as it specialises for FT-133.

## Acceptance

- Worker exits with code 0.
- The verdict's `judges` field is exactly one IRI; that IRI resolves in the bundle to a node typed `dec:AdrProposal`.
- The verdict's `against` field contains exactly two IRIs: the `dec:PreflightGap` IRI and the `dec:FeatureSpec` IRI from the bundle.
- Neither `judges` nor `against` is empty, satisfying the minCount=1 constraints in the ADR-074 SHACL.
- The verdict serialises cleanly through the QualityVerdict pydantic model without missing-field errors.

## Inputs

A synthetic bundle containing exactly one `dec:AdrProposal` IRI, exactly one `dec:PreflightGap` IRI, and exactly one `dec:FeatureSpec` IRI, all typed. The AdrProposal body is approval-grade (so the verdict is approved and the focus stays on shape, not rubric). The mocked Claude response returns a well-formed `QualityVerdict` whose `judges` points at the AdrProposal and whose `against` lists both the gap and the feature_spec.

## Out of scope

- Does not assert rubric-level fields (verdict, rationale, violates) — TC-320 and TC-321 cover approval semantics.
- Does not validate the symmetric contract on the spec-quality side (TC-319 covers that).