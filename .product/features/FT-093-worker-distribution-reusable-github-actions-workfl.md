---
id: FT-093
title: 'worker-distribution: reusable GitHub Actions workflow for releasing workers'
phase: 3
status: planned
depends-on:
- FT-088
- FT-089
- FT-091
- FT-094
adrs:
- ADR-061
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. Every worker repo needs the same release flow: build OCI multi-arch, label, SBOM, push, sign keyless, attach SBOM as referrer, submit. Hosting this once and consuming it via reusable workflow keeps the canonical flow versioned in a single place. Addresses ADR-061 (reusable workflow over per-repo duplication).

## Scope

- Single reusable workflow `release-worker.yml` hosted in pipeline-cli's repo (or a dedicated workflows repo), called from each worker's `.github/workflows/release.yml` on tag push.
- Workflow steps:
  1. Checkout and set up the build environment.
  2. Read the worker's manifest (capability tags, compatible roles, SDK version, wire-protocol version, entrypoint).
  3. Build the OCI image multi-arch via buildx, injecting labels per FT-088.
  4. Generate the CycloneDX SBOM (syft) per FT-091.
  5. Push the image to ghcr.io with the version tag.
  6. `cosign sign` keyless using the ambient GitHub OIDC token (FT-089).
  7. `cosign attach sbom` as an OCI referrer (FT-091).
  8. POST a `WorkerImageSubmission` to pipeline-cli's submission endpoint (FT-094) with registry_ref, capability_tags, compatible_roles, sbom_ref, signed_by identity, provenance.
- Worker manifest TOML shape (declarative; proposed):
  ```toml
  [worker]
  name = "implementer"
  sdk_version = "0.3.0"
  wire_protocol = "1.0"

  [capabilities]
  tags = ["code-writer", "frontier-reasoning"]
  compatible_roles = ["engineering.implementer"]

  [runtime]
  kind = "subscribed"    # vs "invoked" if Dagger lands later
  entrypoint = "implementer.main:run"
  ```
  Manifest fields map directly onto `WorkerImageSubmission` fields; the workflow lifts manifest + build outputs into Submission shape.
- Per-worker `.github/workflows/release.yml` becomes a one-screen file pinning to the reusable workflow's version tag (`@v1`).
- Repo layout: monorepo with path-filtered triggers (`workers/<name>/**` changes trigger that worker's release), scoped semver tags (`implementer-v1.2.0`). The workflow shape doesn't change when a worker graduates to its own repo.

## Out of scope

- Cross-CI portability (slice 3+).
- Auto-publishing of worker docs / changelogs (operational concern, not framework discipline).
- Bumping the reusable workflow's version automatically across worker repos (the explicit-opt-in pin is the point).

## Success criteria

- A tag push to a worker repo triggers the workflow, which produces a multi-arch image, signs it keyless, attaches an SBOM referrer, and POSTs a Submission that the orchestration system receives.
- The reusable workflow itself is tagged `@v1` and re-tagged on revisions; worker repos pinning to `@v1` continue to release on the unchanged contract.
