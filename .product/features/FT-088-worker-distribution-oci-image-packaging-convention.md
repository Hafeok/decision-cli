---
id: FT-088
title: 'worker-distribution: OCI image packaging conventions every worker must follow'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-056
- ADR-057
- ADR-013
- ADR-016
- ADR-044
tests:
- TC-130
domains: []
domains-acknowledged:
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-038: No new artifact type introduced by this feature; existing dual-provenance discipline already governs the artifacts written elsewhere in the brief.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-027: No new role registered by this feature.
  ADR-064: LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM.
  ADR-047: Feature does not perform capability-tag-to-entry binding at dispatch time.
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-043: Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-055: Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable.
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-022: No Feedback artifact produced by this feature.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-036: WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself.
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-039: No new motivational predicate introduced by this feature.
  ADR-065: Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
  ADR-004: Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-041: Feature does not write artifacts through GraphWriter; SHACL enforcement is in scope of FT-086 / FT-087 / FT-094.
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-054: Feature does not call LiteLLM; SDK provider substrate is wired in FT-095 / FT-096.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-002: Feature ships infrastructure / packaging conventions, not graph mutations.
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. The catalog discipline only works if every WorkerImage exposes its capability claims, SDK version, and wire-protocol version in a uniform, machine-readable form on the manifest. Addresses ADR-056 (OCI format) and ADR-057 (capability tags as OCI labels).

## Scope

A worker OCI image MUST:

- Carry capability tags as OCI labels: `ddd.capability-tag.<tag>=true` per tag claimed. Machine-readable from the manifest without pulling the image.
- Pin the worker SDK version: `ddd.sdk-version=<semver>`.
- Pin the wire-protocol version: `ddd.wire-protocol=<semver>`.
- Declare a long-running worker entrypoint that opens the SSE connection to pipeline-cli on start, reading endpoint and bearer token from environment variables (per `feature:manual-runtime-stance`).
- Be multi-arch where reasonable (at least `linux/amd64` and `linux/arm64`).
- Carry an OCI annotation pointing to the source repo and commit hash.

Slice 1 ships a base image `pipeline-worker-base:<version>` that worker authors extend. The base bakes in the SDK and the SSE/POST loop; authors add their worker logic and metadata labels.

The `pipeline-cli` SHACL validation for `WorkerImageSubmission` checks that the candidate image's manifest carries the required labels before admission proceeds.

## Out of scope

- Base image build pipeline (provisioned operationally; reusable workflow consumes it).
- WASM runtime variant (`feature:wasm-runtime`, excluded).
- Dagger runtime variant (`feature:dagger-runtime`, excluded per ADR-065).

## Success criteria

- A worker image built per these conventions is queryable for capability tags via `docker manifest inspect` without pulling.
- A worker image missing a required label is rejected by the admission flow before reaching the Curator's bundle.
- The base image documents the convention and is referenced from worker repo templates.
