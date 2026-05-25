---
id: ADR-053
title: Configurable provider endpoint via LITELLM_BASE_URL and LITELLM_API_KEY
status: proposed
features:
- FT-081
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
---

## Context

The SDK's Provider layer (FT-081) calls LiteLLM (ADR-054). Where LiteLLM
lives — localhost in slice 1, sidecar container in slice 2, shared host
behind a load balancer in production — is a deployment decision, not an
SDK decision. The SDK must not bake the topology into code.

## Decision

The Provider layer reads:

- `LITELLM_BASE_URL` — defaults to `http://localhost:4000` for slice-1
  local-host LiteLLM.
- `LITELLM_API_KEY` — virtual key issued by the LiteLLM proxy, no default.

Both are injected at worker process startup via the `pipeline-cli workers
run` env config. Workers never receive a model name, a provider name, or a
provider API key directly.

## Consequences

- **Positive:** Moving the LiteLLM deployment (localhost → sidecar →
  shared host → load balancer) is a single env-var change. No SDK release,
  no worker rebuild.
- **Positive:** Per-environment configuration (dev points at a permissive
  LiteLLM with caching, prod points at a hardened one with cost limits) is
  achieved by the env config in `pipeline-cli workers run`, not by code
  forks.
- **Negative:** Workers depend on LiteLLM being reachable at the configured
  URL — a misconfigured env yields a runtime failure. Mitigated by `pipeline-
  cli workers run` doing a startup connectivity check before spawning
  workers, and by LiteLLM being part of the slice-1 deployment per
  `brief:worker-distribution-slice-1`'s `feature:litellm-proxy-deployment`.

## Alternatives considered

- **Hardcoded `http://localhost:4000`.** Rejected: works for slice 1, breaks
  the moment LiteLLM moves off localhost.
- **Config file shipped with the worker.** Rejected: deployment shape would
  vary per environment, but env vars cover the same space with less
  ceremony and align with `pipeline-cli workers run`'s existing env
  injection.

## References

- `feature:provider-abstraction` (FT-081) reads these env vars.
- ADR-054 (LiteLLM as substrate).
- `brief:worker-distribution-slice-1` / `feature:litellm-proxy-deployment`
  (separate Brief; provides the LiteLLM deployment itself).