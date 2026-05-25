---
id: ADR-057
title: Capability tags carried as OCI labels for manifest-level discovery
status: accepted
features:
- FT-088
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:ae5c385247b9b23ca331b1da35ba750bdd0d8d0070a75dcb77fe47d2f8035635
---

## Context

The orchestration system needs to discover which capability tags a candidate WorkerImage claims without pulling the image content. Catalog scans, policy evaluation, and submission ingestion all need cheap manifest-only reads.

OCI labels are queryable from the image manifest via `docker manifest inspect` (or registry-native APIs) — no image pull, no large transfer.

## Decision

Workers declare capability tags as OCI labels of the form `ddd.capability-tag.<tag>=true`, one label per claimed tag. The image manifest is the authoritative source for the catalog's shallow operations (find images claiming tag X, list all tags claimed by image Y).

The same convention covers other declared metadata: `ddd.sdk-version=<semver>`, `ddd.wire-protocol=<semver>`, source repo, commit hash.

This is a **soft claim** — the WorkerCurator still verifies the labels against actual worker behaviour during conformance audit before admitting the image. The label is the claim; conformance is the proof.

## Consequences

- **Positive:** Capability discovery is a manifest read, not an image pull. Catalog operations stay cheap.
- **Positive:** Standard OCI tooling reads the labels; no DDD-specific reader needed.
- **Positive:** Misclaimed tags surface during the Curator's audit (slice 1: manual; slice 2+: automated replay), not silently in production.
- **Negative:** Labels are mutable in re-tags; the trust path runs through the image digest plus its signature, not the label string. Acceptable — labels are the index, signatures are the gate.

## Alternatives considered

- **Capability tags in a sidecar artifact** (separate registry entry). Rejected — extra round-trip, weaker coupling to the image.
- **Capability tags only inside the image filesystem** (e.g., `/etc/ddd/capabilities`). Rejected — requires pulling the image to read.

## References

- `brief:worker-distribution-slice-1`
