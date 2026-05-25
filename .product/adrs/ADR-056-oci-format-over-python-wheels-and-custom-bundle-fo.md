---
id: ADR-056
title: OCI format over Python wheels and custom bundle for worker packaging
status: proposed
features:
- FT-088
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
---

## Context

Worker code must be packaged so the orchestration system can sign it, verify it, and run it under policy. Three formats were evaluated:

- **Python wheels** — language-specific, no system dependencies captured, no signed-content model that fits the catalog discipline.
- **OCI containers** — capture all dependencies, language-agnostic, universal signing layer (sigstore), registry infrastructure exists everywhere.
- **Custom DDD bundle format** — invented complexity, no ecosystem, no tooling.

## Decision

Worker packaging is OCI containers. Worker repositories build multi-arch OCI images, push them to a registry (slice 1: ghcr.io), and sign them with sigstore.

OCI also leaves room for the Dagger option later (Dagger modules ride on OCI) without committing — see `adr:dagger-deferred` (ADR-065).

## Consequences

- **Positive:** One packaging format covers any language a worker is written in.
- **Positive:** Sigstore (cosign + Fulcio + Rekor) integrates cleanly because it operates on OCI artifacts.
- **Positive:** All system dependencies the worker uses ship with the image; reproducibility is a function of the base image plus the OCI digest.
- **Negative:** Image size larger than a wheel. Acceptable — registries handle layered storage and the operational footprint is dominated by what the worker actually links, not the format.
- **Negative:** Requires container runtime (docker / podman) on the host running workers. Acceptable in slice 1 because the operator is already running container-based infrastructure.

## Alternatives considered

- **Wheels:** rejected (above).
- **Custom DDD bundle:** rejected (above).

## References

- `brief:worker-distribution-slice-1`
- ADR-065 (Dagger deferred — OCI keeps the door open).
