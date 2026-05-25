---
id: ADR-063
title: Worker secrets via env vars sourced from a local config file in slice 1
status: accepted
features:
- FT-095
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:86c74eeac3a2084da3c96a840a524efc5f573894c6c34cc397d16f116b1bc98b
---

## Context

Worker processes in slice 1 need two secrets: the pipeline-cli bearer token (worker → harness auth) and the LiteLLM virtual key (worker → LiteLLM auth, scoped to specific model groups). Provider API keys (Anthropic, OpenAI, Scaleway, etc.) are NOT in this list — they live in LiteLLM's config per ADR-064, not in worker env.

Options for the two secrets workers do need:

- **Env vars at container start, sourced from a local config file.** Simplest, works with `docker run --env-file`, no infrastructure dependency. Visible in process env and `docker inspect`; acceptable for single-operator local deployments.
- **Docker / Kubernetes secrets.** Better than env vars but tied to a specific runtime; doesn't transparently work across docker / podman / k8s.
- **External secrets manager** (Vault, AWS/GCP Secret Manager). The production answer for multi-tenant; introduces a runtime dependency and operational surface that's overkill for slice 1.

## Decision

Slice 1 uses env vars sourced from a local config file (`~/.pipeline-cli/workers.env` by default; overridable). The `pipeline-cli workers run` subcommand reads the file and passes its variables to `docker run` via `--env-file`.

The narrower trust surface — virtual keys with budgets and scope, not raw provider keys — is one of the wins from pulling LiteLLM into slice 1 rather than deferring it. Workers cannot leak provider keys they never had.

Secrets manager option tracked under `feature:secrets-manager-integration` (slice 2+ alternative when scope grows).

## Consequences

- **Positive:** Zero infrastructure dependency for slice 1. Works on any laptop with docker.
- **Positive:** Worker processes never see provider API keys. Compromise of a worker leaks the virtual key (revocable in LiteLLM) and the pipeline-cli bearer token, not the underlying provider key.
- **Negative:** Env vars are visible in process listings and `docker inspect`. Trust model assumes single-operator, trusted host. Breaks under multi-operator, untrusted images, or remote hosting — addressed by `feature:multi-tenant-litellm` (slice 3+) and `feature:secrets-manager-integration` (slice 2+ alternative).
- **Negative:** Rotation is manual. The operator edits `workers.env` and restarts the worker container.

## Alternatives considered

- **Docker secrets / k8s secrets:** rejected — runtime-specific.
- **External secrets manager:** deferred (above).

## References

- `brief:worker-distribution-slice-1`
- ADR-064 (LiteLLM as LLM proxy — provider keys live there, not here).
- `ack:env-var-secret-trust-model` — the trust model assumed.
