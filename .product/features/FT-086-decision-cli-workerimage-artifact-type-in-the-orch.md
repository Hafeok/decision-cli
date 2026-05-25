---
id: FT-086
title: 'decision-cli: WorkerImage artifact type in the orchestration catalog'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-055
- ADR-013
- ADR-016
- ADR-044
- ADR-036
- ADR-038
- ADR-039
- ADR-041
- ADR-043
- ADR-047
tests:
- TC-128
domains: []
domains-acknowledged:
  ADR-054: Feature does not call LiteLLM; SDK provider substrate is wired in FT-095 / FT-096.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-065: Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model.
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-064: LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM.
  ADR-004: Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts.
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-027: No new role registered by this feature.
  ADR-022: No Feedback artifact produced by this feature.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-002: Feature ships infrastructure / packaging conventions, not graph mutations.
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. As soon as more than one worker exists — or more than one version, or workers from more than one author — the orchestration system needs a registration mechanism so policy can reason about which workers are eligible for which capability tags. The Model catalog (impl doc §9) already solves the analogous problem for LLM models; this Feature adds the parallel catalog entry for workers.

Addresses ADR-055 (WorkerImage mirrors the Model catalog).

## Scope

- SHACL shape for `WorkerImage` parallel to `Model`:
  - `id`, `name`, `version`
  - `registry_ref` — OCI reference with digest
  - `capability_tags` — set of strings
  - `compatible_roles` → `Role[]`
  - `signed_by` — sigstore Fulcio cert subject + issuer
  - `sbom_ref` — OCI referrer URI
  - `conformance_audits` → `ConformanceAudit[]`
  - `eligibility_status` — `qualified | candidate | deprecated | pulled`
  - `provenance` — source repo URI, commit hash, GitHub Actions run URL
- Conforms to dual-provenance discipline (mechanical via FT-069; motivational via FT-070). A `WorkerImage`'s motivational origin is at least one of: `addresses Feedback`, `decomposes_from Brief`, or `originated_from DiscoveryFinding`.
- `ConformanceAudit` artifact shape (referenced from `WorkerImage`):
  - `class` — `manual-review | automated-replay` (slice 1 only uses `manual-review` per ADR-060).
  - `verdict`, `notes`, `evidence_refs`, mechanical/motivational provenance.
- Catalog operations: list by capability tag, list by eligibility status, fetch by id.
- Policy can bind a capability tag to a qualified `WorkerImage` (same mechanism as Model binding).

## Out of scope

- Automated conformance audit (slice 2+, `feature:automated-conformance-replay`).
- Multi-tenant catalog namespacing (slice 3+, `feature:multi-tenant-registry`).
- Vulnerability scan gates (slice 3+, `feature:vuln-scanning-gate`).

## Success criteria

- A `WorkerImage` written with `eligibility_status=qualified` is discoverable via the catalog query "find qualified images claiming capability tag X."
- SHACL validation rejects a `WorkerImage` missing required fields or motivational provenance.
- Policy can express "tag X is bound to WorkerImage Y@v1.2.0" and dispatch resolves to the running worker process for that image.
- The first `WorkerImage` admitted has its motivational provenance trace back to `brief:worker-distribution-slice-1`.
