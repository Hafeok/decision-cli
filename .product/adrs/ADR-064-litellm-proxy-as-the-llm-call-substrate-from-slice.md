---
id: ADR-064
title: LiteLLM proxy as the LLM-call substrate from slice 1
status: proposed
features:
- FT-096
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
---

## Context

The original brief deferred the LLM proxy to slice 2 under the assumption that building one was significant engineering work. Evaluation surfaced LiteLLM as a mature open-source proxy that solves the same problem: unified OpenAI-shaped API across providers, virtual key management, spend tracking, logging callbacks, fallbacks, retries. Using LiteLLM, "running the proxy" drops to "operating a configured service." The deferral no longer pays off.

This is the deployment side of the framework substrate decision. The worker SDK's provider layer is a thin LiteLLM client (`adr:litellm-as-provider-substrate` / ADR-054 in the pipeline-worker SDK brief); this ADR governs the proxy that the SDK calls.

## Decision

LiteLLM is the LLM proxy from slice 1. Workers route every LLM call through it. Provider API keys live in LiteLLM's config; workers hold only a LiteLLM virtual key.

Authoritative source-of-truth split:

- **pipeline-cli's orchestration graph (session records)** is authoritative for provenance, bundle hash, role, motivational origin, downstream consequences. Workers report their own telemetry in completion events.
- **LiteLLM** is authoritative for rate-limit state, fallback decisions made during a call, and the cost figure (LiteLLM sees actual provider pricing). LiteLLM's logging callback POSTs telemetry to pipeline-cli's reconciliation endpoint; the session record absorbs the cost figure and flags drift against the worker's self-reported telemetry as a fitness signal.

OpenAI-shaped API at the worker layer is acceptable: it's the de facto standard, and provider-specific features (Anthropic tool use, etc.) pass through via LiteLLM's `extra_body` parameter.

## Why this isn't framework lock-in

LiteLLM is a wire-level translator/proxy, not a composition framework. It doesn't impose how work is structured or how agents compose. Analogous to accepting `requests` as an HTTP layer; not analogous to accepting Rails as an app framework.

## Consequences

- **Positive:** Order-of-magnitude less code on the LLM-call path that pipeline-cli has to own. Provider adapters, retry policy, fallbacks, spend tracking, virtual key management — all LiteLLM's concern.
- **Positive:** Adding a new provider is a LiteLLM config edit, not a code change. The provider list becomes deployment configuration.
- **Positive:** Centralised observability across all worker LLM traffic via LiteLLM logs + callbacks.
- **Positive:** Worker env carries only a scoped virtual key, not a raw provider API key — narrower compromise surface (see ADR-063).
- **Negative:** LiteLLM is a runtime dependency on the critical path. If LiteLLM is down, every worker is down. Mitigated by operational simplicity (single binary or container) and HA deployment options.
- **Negative (managed):** Competing source-of-truth — handled by the split above.

## Alternatives considered

- **Build our own LLM proxy from scratch** (the original plan). Duplicates commoditised work; no observability, routing, or cost-tracking wins. Rejected.
- **OpenRouter or similar SaaS proxy.** Same architectural fit, but introduces a third-party runtime dependency on the critical path. Self-hosting LiteLLM keeps the dependency at the open-source library level. Rejected for the SaaS path; LiteLLM-as-self-hosted accepted.
- **LiteLLM as Python SDK only (no proxy server).** In-process use per worker instead of a separate proxy. Loses centralised key management, centralised logging, and the ability for multiple worker processes to share one configured deployment. Rejected; proxy is the right shape.
- **External secrets manager for raw provider keys per worker.** Workers still call providers directly; secrets manager handles rotation. Simpler to adopt; doesn't get call-layer scoping or cost-source unification. Tracked as `feature:secrets-manager-integration` (excluded; alternative path).

## Slice 1 starting point

Ship LiteLLM with one model group (Anthropic via the provider's API). Adding OpenAI / Scaleway / Bedrock / Vertex etc. is a LiteLLM config edit, not a code change anywhere in pipeline-cli or the worker SDK.

## References

- `brief:worker-distribution-slice-1`
- ADR-054 (pipeline-worker SDK: LiteLLM as provider substrate) — the SDK side.
- ADR-063 (Secrets via env in slice 1) — why provider keys live here, not in worker env.
