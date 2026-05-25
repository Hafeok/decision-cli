---
id: ADR-059
title: CycloneDX SBOM attached as an OCI referrer
status: proposed
features:
- FT-091
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
---

## Context

The orchestration system records what's inside every WorkerImage so vulnerability state (slice 3+) and supply-chain audit (any time) can run against authoritative dependency data. SBOM placement options:

- **Embedded in the image filesystem** (e.g., `/usr/share/sbom/cyclonedx.json`). Works, but the SBOM is unreadable without pulling the whole image. Wasteful at catalog-scan time.
- **Stored as a sibling registry entry** (independent OCI artifact). Reachable, but needs an out-of-band convention for "find the SBOM attached to image X."
- **Attached as an OCI referrer per OCI v1.1.** The registry returns "what's attached to this digest" natively, without pulling the image. Standard tooling exists (`cosign attach`, `syft`).

## Decision

Workers produce a CycloneDX SBOM during the release workflow (slice 1: via `syft`) and attach it to the image as an OCI referrer per OCI v1.1. The `sbom_ref` field on `WorkerImage` is the referrer descriptor URI.

Slice 1 makes the SBOM available; it does not gate admission on vulnerability scan results. That's slice 3+ work. The WorkerCurator notes SBOM presence in the conformance audit but does not scan.

## Consequences

- **Positive:** SBOM is discoverable via a registry query rather than an image pull.
- **Positive:** Standard OCI tooling produces and consumes referrers; no DDD-specific format.
- **Positive:** CycloneDX over SPDX picks the format with deeper ecosystem support for the languages worker authors are likely to use.
- **Negative:** Older registries that don't speak OCI v1.1 referrers cannot host the relationship. Acceptable — ghcr.io and the major registries already support v1.1.

## Alternatives considered

- **Embedded SBOM:** rejected (above).
- **Sibling registry entry without referrer relation:** rejected (above).

## References

- `brief:worker-distribution-slice-1`
- OCI Image Specification v1.1 (referrers API).
