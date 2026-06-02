---
id: TC-312
title: Every authored DispatchGroup in complete status has a paired approved QualityVerdict
type: scenario
status: unimplemented
validates:
  features: []
  adrs:
  - ADR-073
  - ADR-017
phase: 1
runner: bash
runner-args: scripts/checks/authored-dispatch-group-paired.sh
runner-timeout: 30
observes:
- exit-code
- graph
---

## Purpose

Cross-cutting fitness function. ADR-073 defines the author/judge worker pair as the unit of dispatch (a DispatchGroup); ADR-017 mandates `prov:wasInformedBy` chains between paired sessions. This check asserts that every authored-pair DispatchGroup whose status is `complete` in the orchestration store has a paired QualityVerdict with `verdict=approved` reachable via the `prov:wasInformedBy` edge. An orphan authored DispatchGroup (complete without an approved judge) means the harness skipped the judge step — an audit-trail hole.

## Acceptance

- The script exits 0 when, for every DispatchGroup with `type=author` and `status=complete` in `.dec/store/orchestration.nq`, a corresponding `quality_verdict` artifact exists, reachable by following `prov:wasInformedBy` forward, and that verdict's `verdict` field equals `approved`.
- The script exits 1 if any complete authored DispatchGroup lacks a paired QualityVerdict OR the paired verdict is not `approved`.
- The script exits 1 (not 2) on missing-pair, per ADR-013's two-tier runner contract.
- On failure, the script prints the offending DispatchGroup IRI(s) and the missing-verdict reason to stderr for operator debugging.

## Inputs

Live repo state: the SPARQL query is executed against `.dec/store/orchestration.nq` via `oxigraph-cli` or `dec sparql`. No fixtures; the script reads whatever the current store contains. A passing local repo with no completed authored dispatches yet trivially exits 0.

## Out of scope

- Asserting per-verdict provenance shape (covered by TC-313).
- Asserting auto-accept routing (covered by TC-314).

