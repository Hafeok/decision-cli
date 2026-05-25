---
id: ADR-065
title: Dagger deferred as worker runtime model
status: accepted
features:
- FT-095
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
content-hash: sha256:361922bdcbda7a4e347a716890be914ecbf9b024507f2a2bb39974c4c01cec61
---

## Context

Dagger (dagger.io) was evaluated as an alternative runtime model for workers: workers as Dagger functions, content-addressed, OCI-distributed by default, with the Dagger engine handling invocation rather than the SSE/POST contract from the pipeline-worker SDK brief.

The fit is asymmetric. Dagger's function model is short-lived RPC; the current worker design is event-subscribed and long-running.

Dagger gains: multi-language SDKs, free content-addressed caching, baked-in OCI distribution.

Costs: substrate dependency (open-source but commercially driven), cold-start per dispatch, inverted invocation flow, stateless worker shape only, adopted Dagger conventions and idioms across every worker.

## Decision

Stay with OCI + sigstore + the SSE/POST SDK contract. Dagger is not adopted in slice 1.

OCI as the packaging format (ADR-056) deliberately leaves the Dagger door open — Dagger modules ride on OCI — without committing.

## Doors left open

- **Hybrid runtime model.** Dagger as a second runtime stance alongside subscribed workers, distinguished by a `runtime_kind` field on `WorkerImage` (`subscribed` vs `invoked`). Reconsider when a worker shape appears that genuinely prefers RPC over subscription (stateless pure-execution, high-fanout classifiers).
- **Dagger as conformance-audit runtime.** Even if production dispatch stays on SSE/POST, Dagger's content-addressed caching makes it a clean fit for conformance-replay infrastructure. Worth evaluating when the conformance corpus exists and automated replay is being built (slice 2+).

Both are listed as feature exclusions in the brief, deliberately, so re-entering the decision later finds the prior reasoning attached.

## Consequences

- **Positive:** No commitment to a runtime substrate the framework can't change without lock-in cost.
- **Positive:** The long-running subscribed-worker shape (which suits LLM-call latency profiles well) is the default.
- **Negative:** Workers in languages without a comfortable HTTP-SSE story pay a slightly higher integration cost than they would with Dagger SDKs. Mitigated by the SDK Brief shipping a Python SDK first and the wire protocol being trivial.

## Alternatives considered

- **Adopt Dagger as the only runtime in slice 1.** Rejected — inverted invocation flow doesn't fit subscribed long-running workers; substrate commitment is too heavy for slice 1.
- **Adopt Dagger as the secondary runtime in slice 1.** Rejected — second runtime ahead of evidence that any worker actually wants RPC.

## References

- `brief:worker-distribution-slice-1`
- ADR-056 (OCI format — preserves the Dagger door).
- `feature:dagger-runtime` (excluded).
