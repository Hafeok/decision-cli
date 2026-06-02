---
id: TC-298
title: spec-author returns new with H2/H3-conforming body for a well-formed request
type: scenario
status: unimplemented
validates:
  features:
  - FT-129
  - FT-134
  adrs:
  - ADR-073
  - ADR-074
  - ADR-047
phase: 1
runner: pytest
runner-args: workers/spec-author/tests/test_new_conforming.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Validates FT-129 (spec-author worker). For a well-formed request bundle, spec-author returns `kind: "new"` with a body containing every H2 section required by ADR-047's feature-spec shape (Description, Functional Specification, Out of scope) AND every H3 subsection under Functional Specification (Inputs, Outputs, State, Behaviour, Invariants, Error handling, Boundaries). This is the green path for spec authoring.

## Acceptance

- Parsed stdout deserialises to a `SpecProposal` whose `kind` equals `"new"`.
- The proposal's `new.body` contains all three required H2 headers (case-insensitive substring match on `"## Description"`, `"## Functional Specification"`, `"## Out of scope"`).
- The body contains all seven required H3 subsections under Functional Specification (`### Inputs`, `### Outputs`, `### State`, `### Behaviour`, `### Invariants`, `### Error handling`, `### Boundaries`).
- The proposal echoes the input `bundle_hash`.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: a well-formed `SpecRequest` carrying a title, scope, problem statement, and at least one acceptance criterion. The Anthropic client is monkeypatched to return a `SpecProposal(kind="new", new=ProposedSpec(body=<full conforming markdown>))`.

## Out of scope

- Gap path (covered by TC-299) and acknowledgement-equivalent variants.
- Validator-side W030 absence (covered by TC-300).

