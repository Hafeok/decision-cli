---
id: TC-293
title: tc-quality verdict carries dec:judges and dec:against per ADR-074 polymorphism
type: scenario
status: passing
validates:
  features:
  - FT-127
  adrs:
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/tc-quality/tests/test_judges_against.py
runner-timeout: 60
observes:
- exit-code
- stdout
- graph
last-run: 2026-06-04T12:00:11.713806291+00:00
last-run-duration: 0.3s
---

## Purpose

Validates FT-127 (tc-quality worker) against ADR-074's polymorphic provenance contract for QualityVerdict. The verdict must carry `dec:judges` (the IRI of the judged TcProposal) and `dec:against` (the IRI list of artifacts being judged against — for tc-quality this includes the feature_spec). The SHACL shape from ADR-074 requires both to be non-empty and to resolve to the right artifact classes.

## Acceptance

- Parsed stdout `QualityVerdict.judges` resolves to a `TcProposal`-class IRI present in the input bundle.
- Parsed stdout `QualityVerdict.against` is a non-empty list containing exactly the feature_spec IRI from the input bundle.
- The IRIs serialise as well-formed strings (validatable by an `IriRef` regex).
- The verdict echoes the input `bundle_hash`.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: a `TcProposal` with known IRI (`dec:TcProposal/abc123`) and a feature_spec with known IRI (`dec:FT-stub`). The Anthropic client is monkeypatched to return a `QualityVerdict(judges="dec:TcProposal/abc123", against=["dec:FT-stub"], ...)`. The test asserts on the parsed JSON fields directly; no real graph store is required.

## Out of scope

- The downstream graph-side SHACL validation (covered by harness-level TCs).
- Verdict kind (approved/rejected/amendment-required); this TC only asserts provenance shape.