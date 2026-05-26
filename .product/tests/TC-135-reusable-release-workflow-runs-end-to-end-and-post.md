---
id: TC-135
title: Reusable release workflow runs end-to-end and posts a WorkerImageSubmission
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_135_reusable_release_workflow_runs_end_to_end_and_post
runner-timeout: 120
last-run: 2026-05-26T01:39:23.742247551+00:00
last-run-duration: 0.4s
---

## Description

Exit criterion for [FT-093](FT-093): the reusable release workflow shipped
as `.github/workflows/release-worker-full.yml` performs the full slice-1
worker-release flow (build OCI multi-arch → push → sign keyless → attach
SBOM referrer → POST `WorkerImageSubmission`) and the resulting
Submission is admitted by the orchestration substrate.

The actual GitHub Actions workflow cannot be exercised at unit-test
latency (per the same convention TC-133 documents for FT-091's signing
primitive). This checkpoint test instead pins the *contract* the
workflow produces — the field-mapping from a `worker.toml` manifest and
the workflow's build outputs to the `POST /submissions` JSON body — and
verifies that the admission substrate accepts the result end-to-end
through axum.

## Given

A canonical FT-093 worker manifest fixture
(`tests/data/ft_093_worker_manifest.toml`) mirroring the template at
`docs/templates/worker.toml`. A `ReleaseBuildOutputs` value standing in
for the workflow's `buildx push` digest, `cosign attach sbom` referrer
URI, `cosign sign --keyless` identity, and GitHub Actions provenance
fields. An in-process axum harness running the FT-094 submissions
service over an in-memory Oxigraph store, with a bearer token bound to
the manifest's declared source repo.

## When

```bash
cargo test -p decision-cli --test tc_135_reusable_release_workflow_runs_end_to_end \
  tc_135_reusable_release_workflow_runs_end_to_end_and_post
```

## Then

The checkpoint test asserts six structural claims end-to-end:

1. The canonical manifest fixture parses cleanly into a
   `core::worker_manifest::WorkerManifest` whose `[worker].name`,
   `[runtime].kind`, capability tags, and compatible-role IRIs match the
   FT-093 contract.
2. `core::worker_manifest::assemble_submission_payload` lifts
   `(manifest + ReleaseBuildOutputs)` into a `SubmissionPayloadFields`
   whose serialised JSON shape deserialises into
   `features::submissions::SubmissionPayload` without loss — i.e. the
   workflow's curl-built body and the typed payload share one wire
   shape.
3. POSTing the assembled payload to `/submissions` returns 200 with a
   `submission_id` + `dispatch_event_id`, lands a
   `dec:WorkerImageSubmission` in the orchestration graph, and the
   dispatch target role is `worker-curator`.
4. The persisted Submission carries every manifest-derived capability
   tag in `dec:claimed_capability_tag` and threads the FT-089 sigstore
   identity, FT-091 SBOM referrer URI, and FT-088 provenance fields
   verbatim.
5. The reusable workflow `.github/workflows/release-worker-full.yml`
   exists and declares the FT-093 primitive set: `workflow_call`
   trigger, `worker_name` / `worker_manifest_path` /
   `worker_dockerfile_path` / `image_repo` / `submission_endpoint`
   inputs, `submission_token` secret, multi-arch `docker buildx build`,
   ghcr.io push, delegation to the FT-089/FT-091 signing primitive
   workflow, and the bearer-authorised `POST /submissions` step.
6. The consumer template `docs/templates/release.yml` pins
   `release-worker-full.yml@v1` per ADR-061's explicit-opt-in versioning
   contract and threads the `PIPELINE_SUBMISSION_TOKEN` secret + the
   OIDC `id-token: write` permission.

Exit 0 = pass. Any failure surfaces with the claim that failed.