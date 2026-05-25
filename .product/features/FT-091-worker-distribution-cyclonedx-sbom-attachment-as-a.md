---
id: FT-091
title: 'worker-distribution: CycloneDX SBOM attachment as an OCI referrer'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-059
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. Recording what's inside every WorkerImage in a queryable, registry-resident form is a prerequisite for slice 3+ vulnerability gates and any later supply-chain audit. Addresses ADR-059 (SBOM as OCI referrer).

## Scope

- The release workflow generates a CycloneDX SBOM for the built image (slice 1: via `syft`).
- The SBOM is attached to the image as an OCI referrer per OCI v1.1 (slice 1: via `cosign attach sbom`).
- The `WorkerImageSubmission`'s `sbom_ref` field carries the referrer descriptor URI; the admitted `WorkerImage` propagates it.
- The Curator's bundle includes the SBOM reference (the referrer descriptor, not the SBOM body); the SBOM is reachable on demand for human inspection but not pre-fetched.
- Slice 1 does not scan the SBOM for vulnerabilities. The Curator notes presence and references it in the admission verdict.

## Out of scope

- Vulnerability scanning and gating (slice 3+, `feature:vuln-scanning-gate`).
- SBOM format alternatives (SPDX) — CycloneDX chosen for ecosystem depth.
- Periodic re-scan of admitted WorkerImages against updated vulnerability feeds (slice 3+).

## Success criteria

- An image built via the release workflow has an attached CycloneDX SBOM referrer that `cosign download sbom <image-ref>` resolves to.
- The Submission's `sbom_ref` is validated by SHACL as a syntactically-correct OCI referrer descriptor.
- The Curator's bundle exposes the SBOM reference; bundle assembly fails when the SBOM is declared missing on a Submission.
