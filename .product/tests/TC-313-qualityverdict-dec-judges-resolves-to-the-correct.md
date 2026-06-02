---
id: TC-313
title: QualityVerdict dec:judges resolves to the correct artifact class per judged kind (polymorphism)
type: scenario
status: unimplemented
validates:
  features: []
  adrs:
  - ADR-074
phase: 1
runner: bash
runner-args: scripts/checks/quality-verdict-polymorphism.sh
runner-timeout: 30
observes:
- exit-code
- graph
---

## Purpose

Cross-cutting fitness function. ADR-074 defines QualityVerdict polymorphism: `dec:judges` must resolve to an artifact whose class matches the verdict's judged kind — TcProposal → TestCriterion (or TcProposal itself), GraphProposal → VerificationGraph (or GraphProposal), SpecProposal → FeatureSpec (or SpecProposal), AdrProposal → ADR (or AdrProposal). A misrouted judges edge means the verdict is judging the wrong thing — a structural bug that breaks downstream auto-accept routing.

## Acceptance

- The script exits 0 when, for every QualityVerdict in `.dec/store/orchestration.nq`, the IRI on the right-hand side of `dec:judges` `rdf:type`s to one of the allowed classes for that verdict's judged kind per the ADR-074 mapping table.
- The script exits 1 if any QualityVerdict has a `dec:judges` resolving to a class outside its allowed set (e.g. a tc-quality verdict judging a GraphProposal).
- The script exits 1 (not 2) on a mismatch, per ADR-013 two-tier contract.
- On failure, the script prints the offending verdict IRI, the actual judged class, and the expected class set to stderr.

## Inputs

Live repo state: SPARQL query against `.dec/store/orchestration.nq` enumerating all QualityVerdict instances and their `dec:judges` targets. A passing local repo with no verdicts yet trivially exits 0.

## Out of scope

- Whether a paired verdict exists at all (covered by TC-312).
- Whether the verdict triggered the right auto-accept transition (covered by TC-314).

