---
id: TC-300
title: spec-author proposal body passes FT-055/ADR-047 body-completeness validation
type: scenario
status: unimplemented
validates:
  features:
  - FT-129
  - FT-134
  adrs:
  - ADR-073
  - ADR-047
phase: 1
runner: pytest
runner-args: workers/spec-author/tests/test_body_schema.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Validates FT-129 (spec-author worker) against ADR-047's body-completeness contract as enforced by the FT-055 validator. The proposed `new.body` must pass the same body-completeness check the harness runs at ingest, so a freshly authored spec lands without W030 warnings — closing the loop between authoring and validation.

## Acceptance

- The FT-055 body-completeness validator, invoked on the `SpecProposal.new.body` string, returns zero W030 warnings.
- The validator's `errors` collection is empty.
- The validator's section coverage report shows all required H2/H3 markers present.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON with a complete `SpecRequest`. The Anthropic client is monkeypatched to return a `SpecProposal(kind="new", new=ProposedSpec(body=<full conforming markdown matching ADR-047>))`. The test then imports the FT-055 validator entry point (or invokes it as a subprocess) and feeds the proposed body to it.

## Out of scope

- Behavioural assertions on which body content was produced (TC-298 covers structural presence; this TC closes the loop with the validator).
- Validator behaviour on already-stored feature_specs (covered by FT-055 TCs).

