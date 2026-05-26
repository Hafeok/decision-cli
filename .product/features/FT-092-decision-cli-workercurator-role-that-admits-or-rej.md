---
id: FT-092
title: 'decision-cli: WorkerCurator role that admits or rejects WorkerImageSubmissions'
phase: 3
status: complete
depends-on:
- FT-087
- FT-090
- FT-091
adrs:
- ADR-060
- ADR-013
- ADR-016
- ADR-044
- ADR-022
- ADR-023
- ADR-024
- ADR-025
- ADR-027
- ADR-035
- ADR-038
- ADR-039
- ADR-041
tests:
- TC-134
domains: []
domains-acknowledged:
  ADR-064: LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM.
  ADR-002: Feature ships infrastructure / packaging conventions, not graph mutations.
  ADR-065: Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model.
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-055: Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-047: Feature does not perform capability-tag-to-entry binding at dispatch time.
  ADR-004: Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts.
  ADR-054: Feature does not call LiteLLM; SDK provider substrate is wired in FT-095 / FT-096.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-036: WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself.
  ADR-043: Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces.
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
