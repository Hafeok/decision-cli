---
id: FT-093
title: 'worker-distribution: reusable GitHub Actions workflow for releasing workers'
phase: 3
status: complete
depends-on:
- FT-088
- FT-089
- FT-091
- FT-094
adrs:
- ADR-061
- ADR-013
- ADR-016
- ADR-044
tests:
- TC-135
domains: []
domains-acknowledged:
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-041: Feature does not write artifacts through GraphWriter; SHACL enforcement is in scope of FT-086 / FT-087 / FT-094.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-038: No new artifact type introduced by this feature; existing dual-provenance discipline already governs the artifacts written elsewhere in the brief.
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-064: LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM.
  ADR-036: WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself.
  ADR-054: Feature does not call LiteLLM; SDK provider substrate is wired in FT-095 / FT-096.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
  ADR-022: No Feedback artifact produced by this feature.
  ADR-027: No new role registered by this feature.
  ADR-055: Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable.
  ADR-047: Feature does not perform capability-tag-to-entry binding at dispatch time.
  ADR-043: Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces.
  ADR-065: Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model.
  ADR-039: No new motivational predicate introduced by this feature.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-002: Feature ships infrastructure / packaging conventions, not graph mutations.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-004: Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts.
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
