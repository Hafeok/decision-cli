---
id: TC-302
title: adr-author returns new ADR for a gap warranting a net-new decision
type: scenario
status: unimplemented
validates:
  features:
  - FT-130
  adrs:
  - ADR-073
  - ADR-074
phase: 1
runner: pytest
runner-args: workers/adr-author/tests/test_new_adr.py
runner-timeout: 60
observes:
- exit-code
- stdout
---

## Purpose

Validates FT-130 (adr-author worker). When a preflight gap describes a decision that no existing ADR governs and that warrants a net-new ADR, adr-author returns `kind: "new"` with a body containing every required H2 section (Context, Decision, Rejected alternatives, Consequences) and a `scope` field drawn from the controlled enum. This is the green path for the ADR authoring arm.

## Acceptance

- Parsed stdout `AdrProposal.kind` equals `"new"`.
- The proposal's `new.body` contains all four required H2 headers (`"## Context"`, `"## Decision"`, `"## Rejected alternatives"`, `"## Consequences"`).
- The proposal's `new.scope` is in the controlled enum (`"slice"`, `"cross-cutting"`, etc., per ADR-014).
- The proposal echoes the input `bundle_hash`.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON: a preflight gap describing a missing decision (e.g. "no ADR governs how the planner handles oscillation") with no existing ADR that fits. The Anthropic client is monkeypatched to return an `AdrProposal(kind="new", new=ProposedAdr(body=<full ADR markdown>, scope="cross-cutting"))`.

## Out of scope

- Acknowledgement path (covered by TC-303 and TC-304).
- Gap path (covered by TC-305).

