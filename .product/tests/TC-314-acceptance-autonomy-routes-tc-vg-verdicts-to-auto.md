---
id: TC-314
title: Acceptance autonomy routes TC/VG verdicts to auto-flip and spec/ADR verdicts to pending-review
type: scenario
status: unimplemented
validates:
  features: []
  adrs:
  - ADR-075
phase: 1
runner: bash
runner-args: scripts/checks/acceptance-autonomy-routing.sh
runner-timeout: 30
observes:
- exit-code
- graph
---

## Purpose

Cross-cutting fitness function. ADR-075 defines the acceptance autonomy table: approved TC and VG verdicts auto-flip readiness state at the `quality_verdict_accepted` harness transition, while approved spec and ADR verdicts remain in `pending_review` until a human acceptance event fires. This check asserts the auto-acceptance router obeyed those rules — every approved TC/VG verdict in the store flipped readiness, and no approved spec/ADR verdict did.

## Acceptance

- The script exits 0 when every QualityVerdict in the store with `verdict=approved` AND judges-kind in {TcProposal, GraphProposal} has a corresponding state-transition event flipping the relevant readiness flag (tcs_ready or vgs_ready).
- The script exits 0 when every QualityVerdict with `verdict=approved` AND judges-kind in {SpecProposal, AdrProposal} has the corresponding artifact still in `pending_review` (or only flipped by an explicit human-acceptance event, identifiable by actor type).
- The script exits 1 if any TC/VG approved verdict did NOT auto-flip, OR if any spec/ADR approved verdict was auto-flipped (i.e. flipped without a human-acceptance event).
- On failure, the script prints each offending verdict IRI with the actual vs expected routing to stderr.

## Inputs

Live repo state: SPARQL queries against `.dec/store/orchestration.nq` joining QualityVerdict, the judged artifact, and any state-transition events tagged with their actor. A passing local repo with no verdicts yet trivially exits 0.

## Out of scope

- Verdict-shape correctness (covered by TC-313).
- The rolling-window auto-accept reopening-rate fitness function (covered by TC-315).

