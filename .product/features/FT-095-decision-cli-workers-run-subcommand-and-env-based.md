---
id: FT-095
title: 'decision-cli: workers run subcommand and env-based secret handling'
phase: 3
status: complete
depends-on:
- FT-086
- FT-096
adrs:
- ADR-062
- ADR-063
- ADR-065
- ADR-013
- ADR-016
- ADR-044
- ADR-054
tests:
- TC-137
domains: []
domains-acknowledged:
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-027: No new role registered by this feature.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-038: No new artifact type introduced by this feature; existing dual-provenance discipline already governs the artifacts written elsewhere in the brief.
  ADR-043: Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
  ADR-036: WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself.
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-022: No Feedback artifact produced by this feature.
  ADR-047: Feature does not perform capability-tag-to-entry binding at dispatch time.
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-064: LiteLLM substrate concerns are isolated to FT-095 / FT-096; this feature does not call LiteLLM.
  ADR-041: Feature does not write artifacts through GraphWriter; SHACL enforcement is in scope of FT-086 / FT-087 / FT-094.
  ADR-039: No new motivational predicate introduced by this feature.
  ADR-002: Feature ships infrastructure / packaging conventions, not graph mutations.
  ADR-055: Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-004: Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts.
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. Slice 1 doesn't ship a WorkerSupervisor (per ADR-062). The operator IS the supervisor: they read the orchestration system's binding state and start one process per capability tag they want covered. The `workers run` subcommand provides the minimum surface for that, and the env-var secret model (per ADR-063) provides the worker process its two credentials without dragging in infrastructure. ADR-065 (Dagger deferred) governs the runtime substrate decision underpinning this stance.

## Scope

- New subcommand: `pipeline-cli workers run <worker-image-id>`.
  1. Look up the qualified `WorkerImage` by ID in the orchestration catalog; resolve its `registry_ref`.
  2. Pull the image via `docker` (or `podman` — same CLI surface).
  3. Read env vars from `~/.pipeline-cli/workers.env` (overridable by `--env-file <path>`) containing:
     - `PIPELINE_ENDPOINT` — the SSE endpoint URL on pipeline-cli.
     - `PIPELINE_TOKEN` — the worker's bearer token for pipeline-cli auth.
     - `LITELLM_BASE_URL` — the LiteLLM proxy URL (defaults to `http://localhost:4000`).
     - `LITELLM_API_KEY` — the worker's LiteLLM virtual key scoped to specific model groups.
  4. Invoke `docker run --rm --env-file ...` with stdout/stderr attached to the calling terminal.
- Error handling:
  - WorkerImage id not found, or `eligibility_status != qualified` → exit non-zero with explanatory message.
  - Required env vars missing from the file → exit non-zero before pulling.
  - Image pull failure → propagate the docker error.
- Trust model per `ack:env-var-secret-trust-model`: provider keys (Anthropic, OpenAI, etc.) live in LiteLLM's config, not in worker env. Workers cannot leak provider keys they never had.

## Out of scope

- Daemon / restart / autoscale behaviour (`feature:worker-supervisor`, slice 4+).
- `pipeline-cli workers compose` that generates a `docker-compose.yml` from binding state (slice 2-3 progression).
- Remote-host worker hosting (multi-operator / multi-tenant; slice 3+).
- Secrets-manager-backed env vars (`feature:secrets-manager-integration`, slice 2+ alternative).

## Success criteria

- `pipeline-cli workers run <id>` pulls a qualified image and starts a container with the four required env vars set, observably running.
- The running worker opens its SSE subscription to pipeline-cli and receives the next dispatch for its bound capability tag.
- A missing env var, a non-qualified image, or a non-existent image id each produce a clear non-zero exit before any container starts.
