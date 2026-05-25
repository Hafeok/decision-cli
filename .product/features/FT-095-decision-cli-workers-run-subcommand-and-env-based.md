---
id: FT-095
title: 'decision-cli: workers run subcommand and env-based secret handling'
phase: 3
status: planned
depends-on:
- FT-086
- FT-096
adrs:
- ADR-062
- ADR-063
- ADR-065
tests: []
domains: []
domains-acknowledged: {}
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
