---
id: FT-094
title: 'decision-cli: WorkerImageSubmission HTTP endpoint on pipeline-cli'
phase: 3
status: complete
depends-on:
- FT-087
adrs:
- ADR-044
- ADR-039
tests:
- TC-136
domains: []
domains-acknowledged:
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-043: Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces.
  ADR-036: WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself.
  ADR-027: No new role registered by this feature.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-054: Feature does not call LiteLLM; SDK provider substrate is wired in FT-095 / FT-096.
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-055: Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-047: Feature does not perform capability-tag-to-entry binding at dispatch time.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-064: LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM.
  ADR-065: Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model.
  ADR-022: No Feedback artifact produced by this feature.
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. Producer-side CI needs an explicit, authenticated boundary into the orchestration system for posting `WorkerImageSubmission`s. This is the only producer-side intake; all other worker artifacts (WorkerImage, ConformanceAudit, binding policy) are produced inside the orchestration system by its roles.

## Scope

- HTTP endpoint on pipeline-cli: `POST /submissions`. Bearer-token authenticated.
- Authentication model (slice 1):
  - Each worker repo holds a `PIPELINE_SUBMISSION_TOKEN` secret issued by pipeline-cli.
  - Tokens are scoped to that repo's identity (the same identity bound on the trust list per FT-089).
  - Tokens are long-lived in slice 1; rotated manually. Slice 3+ moves token rotation onto the same mechanism that handles worker bearer tokens.
- Request body: a JSON-encoded `WorkerImageSubmission` payload (registry_ref, claimed capability_tags, claimed compatible_roles, sbom_ref, signature_identity, provenance).
- Handler responsibilities:
  - Authenticate the bearer token and resolve the calling repo identity.
  - Validate the submission against the `WorkerImageSubmission` SHACL shape (via existing GraphWriter).
  - Write the Submission as a `BoundaryArtifact` (FT-071) into the orchestration graph.
  - Emit a dispatch event for the WorkerCurator role (FT-092).
  - Respond with the Submission id and the dispatch event id.
- Error paths:
  - `401` for invalid / expired token.
  - `403` for token-identity / declared-source-repo mismatch.
  - `422` for SHACL validation failures (response body lists which fields failed).
  - `429` for repo-scoped rate limit (operational concern; loose default in slice 1).

## Out of scope

- Multi-tenant token namespacing (slice 3+, `feature:multi-tenant-registry`).
- Re-submission idempotency at the Submission level (the producer can rebuild and resubmit; deduplication by image digest is the catalog's concern, not the endpoint's).
- Token rotation automation (slice 3+).

## Success criteria

- A valid `POST /submissions` from an authorised worker repo lands the Submission in the graph and triggers a dispatch event for the WorkerCurator.
- Each error path returns the correct status code and emits no Submission into the graph.
- The endpoint refuses a submission whose declared source repo doesn't match the token's identity.
