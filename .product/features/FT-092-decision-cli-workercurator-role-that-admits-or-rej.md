---
id: FT-092
title: 'decision-cli: WorkerCurator role that admits or rejects WorkerImageSubmissions'
phase: 3
status: planned
depends-on:
- FT-087
- FT-090
- FT-091
adrs:
- ADR-060
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. Every WorkerImage entering the catalog needs an explicit admission decision so the catalog isn't vacuous. Slice 1 has no conformance corpus yet; the decision falls to a human curator with structured supporting evidence. Addresses ADR-060 (manual conformance in slice 1).

## Scope

- Role definition: `WorkerCurator` registered in the role catalog. Autonomy level: 0 (human-filled) in slice 1.
- Bundle assembly query (`curated query helper` per `pipeline-worker-sdk` framing):
  - The focal `WorkerImageSubmission`.
  - The `SignatureVerdict` from FT-090.
  - The SBOM reference from FT-091 (not the SBOM body; reachable on demand).
  - Current orchestration policy: capacity, capability-tag coverage, preferred provenance constraints.
  - Existing `WorkerImage`s with overlapping capability tags (for comparison).
- Output of a Curator session (one of two):
  - **Admission:** writes a `WorkerImage` with `eligibility_status=qualified` plus a `ConformanceAudit` of class `manual-review` (per ADR-060) attached to the new `WorkerImage`'s `conformance_audits` field.
  - **Rejection:** writes a `Feedback` artifact of class `submission-rejected` with evidence pointing at what disqualified the Submission.
- Lifecycle transition on the Submission (`received → admitted` or `received → rejected`) follows from the session output.
- Motivational provenance: every admitted `WorkerImage` traces through its Submission to the Submission's external origin (per BoundaryArtifact / FT-071).

## Out of scope

- Automated conformance replay (slice 2+, `feature:automated-conformance-replay`).
- Multi-Curator workflows / quorum (slice 3+).
- Graduation of the Curator role to higher autonomy levels (slice 4+; needs measurement evidence that doesn't exist yet — see `ack:manual-curator-decisions`).

## Success criteria

- A Curator session given a complete bundle (Submission + valid SignatureVerdict + SBOM present) can be admitted, producing the WorkerImage and ConformanceAudit artifacts atomically.
- A Curator session can reject a Submission, producing a Feedback artifact and leaving no WorkerImage in the catalog.
- The admitted WorkerImage's full motivational chain (`product chain backward FT-...`) terminates at `brief:worker-distribution-slice-1` (via the Submission's external origin).
