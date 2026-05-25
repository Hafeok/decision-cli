---
id: FT-087
title: 'decision-cli: WorkerImageSubmission as the initial-request artifact for admission'
phase: 3
status: planned
depends-on:
- FT-086
adrs: []
tests: []
domains: []
domains-acknowledged: {}
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
