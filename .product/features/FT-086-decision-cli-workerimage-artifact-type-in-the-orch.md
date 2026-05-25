---
id: FT-086
title: 'decision-cli: WorkerImage artifact type in the orchestration catalog'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-055
tests: []
domains: []
domains-acknowledged: {}
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
