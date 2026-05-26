---
id: ADR-054
title: LiteLLM as the worker SDK's provider substrate (no per-provider adapters)
status: accepted
features:
- FT-081
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:7a8a24888df513abdb7e3bc6546aac0cc067e6ce7f17be4b706c2a9e785fe9db
---

## Context

The original plan for the worker SDK's Provider layer was a multi-provider
abstraction the SDK owns: separate Anthropic, OpenAI, Scaleway
implementations conforming to a common interface, plus capability-tag-to-
(provider, model) resolution via a catalog the SDK consumes. This mirrors
what FT-059…FT-066 built on the Rust side for slice 2 of decision-cli.

Evaluation surfaced that this is exactly what LiteLLM already does, with
substantially more features: virtual keys, per-key rate limits, fallbacks,
spend tracking, logging callbacks, response caching, an OpenAI-compatible
unified API across ~100 providers. Building it ourselves duplicates work at
a layer that's already commoditized in the LLM tooling ecosystem.

A key concern: LangChain/AutoGen-style framework lock-in. The DDD stance
(`docs/ddd/Implementing_DDD.md` §2) is "the graph is yours, artifacts are
the interface" — anything that imposes a composition model on workers is
ruled out.

## Decision

Use LiteLLM as the Provider substrate for the worker SDK. The SDK's
`Provider` layer is a thin client of LiteLLM's OpenAI-compatible API. Per-
provider behavior is configured in LiteLLM's deployment, not in worker code.
Capability tags map to LiteLLM model groups.

## Consequences

- **Positive:** Order-of-magnitude less code we own. The SDK doesn't carry
  per-provider adapters, retry policy, fallback logic, rate-limit handling,
  spend tracking, or virtual key management — LiteLLM does.
- **Positive:** Adding a new provider is a LiteLLM config edit, not a
  worker SDK release. The provider list becomes deployment configuration,
  not code.
- **Positive:** Centralized observability. All LLM traffic goes through one
  proxy whose logs and metrics cover every worker, not per-worker
  instrumentation that has to be aggregated.
- **Negative:** LiteLLM is a runtime dependency on the critical path. If
  LiteLLM is down, every worker is down. Mitigated by LiteLLM being
  operationally simple (single binary or container), supporting HA
  deployments, and being part of the slice-1 deployment plan in
  `brief:worker-distribution-slice-1`.
- **Negative (managed):** Competing source-of-truth concern. LiteLLM has its
  own session model (virtual keys with budgets, spend tracking, call logs).
  Resolution: our session record is authoritative for everything DDD cares
  about (provenance, bundle hash, role, motivational origin, downstream
  consequences). LiteLLM's records are operational state for proxy
  concerns (rate limits, fallback decisions, key budgets) and a
  verification feed for cost reconciliation. Where they overlap, our store
  wins. **One explicit exception:** LiteLLM's cost figure is authoritative
  for spend tracking, because LiteLLM sees the actual provider invoice
  line and we don't.

## Why this isn't framework lock-in

LiteLLM is a wire-level translator/proxy, not a composition framework. It
doesn't impose how work is structured, how agents compose, what abstractions
to use for prompting, or how to model multi-turn interactions. The analogy:

- Rejecting LangChain is like rejecting Rails — a framework that defines
  app shape.
- Accepting LiteLLM is like accepting `requests` — a library that handles
  a specific layer (HTTP) well.

No conflict with the "graph is yours, artifacts are the interface" stance.

OpenAI-shaped API at the worker layer is acceptable — it's the de facto
standard, and provider-specific features remain accessible via `extra_body`
passthrough.

## Alternatives considered

- **Per-provider SDK wrappers** (the original plan). More code we own, more
  maintenance, no observability or routing wins. Rejected.
- **OpenRouter or similar SaaS proxy.** Same architectural fit as LiteLLM
  but hosted; introduces a third-party runtime dependency on the critical
  path that we can't control. Self-hosting LiteLLM keeps the dependency at
  the library/service level.
- **LiteLLM-as-library only** (use LiteLLM's Python SDK in-process per
  worker, no proxy server). Loses centralized key management, centralized
  logging, and the fact that multiple worker processes can share one
  running LiteLLM. Rejected; proxy is the right deployment shape.

## Slice 1 starting point

Slice 1 ships LiteLLM with one model group (Anthropic via the provider's
API, since that's what the existing decision-cli workers already use).
Additional providers (OpenAI, Scaleway, Bedrock, etc.) are added by editing
LiteLLM's config, not by SDK changes.

## References

- `feature:provider-abstraction` (FT-081) is the SDK consumer.
- ADR-047 (capability-tag binding) — capability tags map to LiteLLM model
  groups.
- ADR-052 (instructor + Pydantic) — structured output layered on LiteLLM's
  OpenAI-compatible API.
- ADR-053 (configurable provider endpoint) — how the SDK locates LiteLLM.
- `brief:worker-distribution-slice-1` (separate Brief) — owns the LiteLLM
  deployment itself.