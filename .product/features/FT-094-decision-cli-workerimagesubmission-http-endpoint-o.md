---
id: FT-094
title: 'decision-cli: WorkerImageSubmission HTTP endpoint on pipeline-cli'
phase: 3
status: planned
depends-on:
- FT-087
adrs: []
tests: []
domains: []
domains-acknowledged: {}
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
