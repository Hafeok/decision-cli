---
id: FT-091
title: 'worker-distribution: CycloneDX SBOM attachment as an OCI referrer'
phase: 3
status: complete
depends-on: []
adrs:
- ADR-059
- ADR-013
- ADR-016
- ADR-044
tests:
- TC-133
domains: []
domains-acknowledged:
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-055: Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable.
  ADR-038: No new artifact type introduced by this feature; existing dual-provenance discipline already governs the artifacts written elsewhere in the brief.
  ADR-054: Feature does not call LiteLLM; SDK provider substrate is wired in FT-095 / FT-096.
  ADR-002: Feature ships infrastructure / packaging conventions, not graph mutations.
  ADR-004: Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts.
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-047: Feature does not perform capability-tag-to-entry binding at dispatch time.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-036: WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself.
  ADR-064: LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-039: No new motivational predicate introduced by this feature.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
  ADR-043: Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-041: Feature does not write artifacts through GraphWriter; SHACL enforcement is in scope of FT-086 / FT-087 / FT-094.
  ADR-065: Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model.
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-022: No Feedback artifact produced by this feature.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-027: No new role registered by this feature.
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
