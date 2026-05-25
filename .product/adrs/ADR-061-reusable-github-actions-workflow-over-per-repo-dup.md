---
id: ADR-061
title: Reusable GitHub Actions workflow over per-repo duplication
status: proposed
features:
- FT-093
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
---

## Context

Every worker repo needs the same release flow: build OCI multi-arch, label, SBOM, push, sign keyless, attach SBOM as referrer, submit. GitHub Actions supports three ways to share this:

- **Reusable workflow centrally hosted**, called from each worker's tiny per-repo `release.yml`. One short file per worker; canonical flow versioned in one place; updates happen once.
- **Per-repo workflows duplicated.** Each repo owns its release flow. Easy to customise per worker, hard to keep consistent.
- **Composite actions instead.** Composable building blocks rather than a full workflow. More flexibility per consumer; more boilerplate per consumer.

## Decision

The release flow is a single reusable workflow (`release-worker.yml`) hosted in pipeline-cli's repo (or a dedicated workflows repo), called from each worker's `.github/workflows/release.yml` on tag push. The reusable workflow is versioned by tag (`@v1`, `@v2`) and worker repos pin to a version, opting into updates explicitly.

A starter version of the reusable workflow ships alongside the brief.

The reusable workflow is itself a versioned artifact with provenance and change history; revisions to it follow the same discipline as anything else.

## Consequences

- **Positive:** Workers' release files are short and uniform; the canonical flow lives in one place.
- **Positive:** Updating the flow (e.g. adding vulnerability gates in slice 3) is a workflow edit + tag bump; consumers move when ready.
- **Positive:** Provenance of the workflow itself is auditable via its version tag.
- **Negative:** Tight coupling between every worker release and the reusable workflow's availability. Acceptable — the workflow is open-source and forkable.
- **Negative:** GitHub-platform-specific. Cross-CI portability (GitLab, Buildkite) is a slice 3+ concern; the underlying primitives (OIDC, cosign, registry) are platform-portable, only the workflow file isn't.

## Alternatives considered

- **Per-repo workflows duplicated:** rejected (above).
- **Composite actions only:** rejected — pushes the assembly burden to every worker repo.

## References

- `brief:worker-distribution-slice-1`
- ADR-058 (Cosign keyless signing — the workflow's signing step).
