---
id: FT-087
title: 'decision-cli: WorkerImageSubmission as the initial-request artifact for admission'
phase: 3
status: planned
depends-on:
- FT-086
adrs:
- ADR-013
- ADR-016
- ADR-044
- ADR-002
- ADR-038
- ADR-039
- ADR-040
- ADR-041
tests:
- TC-129
domains: []
domains-acknowledged:
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-054: Feature does not call LiteLLM; SDK provider substrate is wired in FT-095 / FT-096.
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-064: LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM.
  ADR-036: WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself.
  ADR-055: Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable.
  ADR-047: Feature does not perform capability-tag-to-entry binding at dispatch time.
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-004: Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
  ADR-065: Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model.
  ADR-022: No Feedback artifact produced by this feature.
  ADR-027: No new role registered by this feature.
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-043: Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces.
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. When a worker author releases a new version, their CI produces a request artifact that the orchestration system's WorkerCurator role consumes. This is the boundary artifact between the producer-side world (worker repo, CI) and the orchestration system's catalog. Without it, admission has no input shape.

## Scope

- SHACL shape for `WorkerImageSubmission` carrying the claim payload:
  - `candidate_registry_ref` — proposed OCI reference with digest.
  - `claimed_capability_tags`, `claimed_compatible_roles`.
  - `sbom_ref` — OCI referrer URI (per FT-091).
  - `signature_identity` — Fulcio cert subject and issuer (per FT-089).
  - `provenance` — source repo URI, commit hash, GitHub Actions run URL.
- Classification as a `BoundaryArtifact` (per ADR-040 / FT-071): the Submission has no upstream motivational origin in the orchestration graph itself — its origin lives in the producer's repo / CI.
- Submission lifecycle states: `received | under-review | admitted | rejected`. Curator session output transitions the state.
- Edges:
  - `produced_workerimage → WorkerImage` (on admission).
  - `produced_feedback → Feedback` (on rejection, class `submission-rejected`).

## Out of scope

- The HTTP endpoint that receives Submissions (`FT-094`).
- The Curator session itself (`FT-092`).
- Re-submission flow after rejection (slice 2+; for now, the producer rebuilds and submits a new Submission).

## Success criteria

- A `WorkerImageSubmission` written via `GraphWriter` is accepted only if it conforms to the SHACL shape and carries valid mechanical provenance.
- The Submission is correctly classified as a `BoundaryArtifact`; FT-073's GraphWriter enforcement does not reject it for missing motivational provenance.
- Lifecycle transitions are reachable only via Curator session output; manual state edits are rejected.
