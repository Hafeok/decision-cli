---
id: FT-081
title: 'pipeline-worker SDK: LiteLLM client with capability-tag dispatch and structured output'
phase: 3
status: planned
depends-on:
- FT-078
adrs:
- ADR-054
- ADR-047
- ADR-052
- ADR-053
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. The SDK's provider layer is a
thin LiteLLM client, not a multi-provider abstraction layer the SDK owns.
Per-provider behavior is configured in the LiteLLM proxy deployment, not in
worker code. Addresses ADR-054 (LiteLLM as substrate), ADR-047 (capability-tag
binding), ADR-052 (instructor for structured output), ADR-053 (configurable
endpoint).

## Location

`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/provider/` —
`litellm_client.py` is the LiteLLM wrapper, `instructor_adapter.py` layers
Pydantic structured-output on top.

## Scope

- `provider.complete(capability_tag, messages, output_schema, metadata)`:
  - `capability_tag` is mapped to a LiteLLM model group (configured in the
    proxy by `brief:worker-distribution-slice-1`'s
    `feature:litellm-proxy-deployment`); workers never see model names.
  - `output_schema` is a Pydantic model; structured output is enforced via
    instructor on top of LiteLLM.
  - `metadata` carries the DDD session ID through to LiteLLM's logging
    callbacks for end-to-end correlation.
- Endpoint configuration:
  - `LITELLM_BASE_URL` (default `http://localhost:4000` for slice-1 local-host
    LiteLLM)
  - `LITELLM_API_KEY` (virtual key issued by the proxy)
  - Both injected via the `pipeline-cli workers run` env config.
- Telemetry capture, two layers:
  - **Synchronous (worker-side, authoritative for provenance):** tokens,
    latency, model, retry count, provider chosen (when LiteLLM routes or
    falls back) — attached to the session's telemetry block.
  - **Asynchronous (LiteLLM callback, authoritative for spend specifically):**
    LiteLLM POSTs to pipeline-cli's `/llm-call-telemetry` endpoint with the
    same fields plus cost (computed from token counts and provider pricing).
    Pipeline-cli reconciles against the worker-reported telemetry; mismatches
    are a fitness function on the proxy.
- Provider-specific parameters reachable via `extra_body` (Anthropic tool use,
  OpenAI `response_format`, etc.) — the SDK doesn't add per-provider methods.

## Out of scope

- Per-provider SDK wrappers (rejected in ADR-054).
- Tool-use multi-turn lineage in a single session (deferred to slice 3 with
  the implementer role; LiteLLM supports it, the SDK surface for it is later
  work).
- LiteLLM-as-library mode (rejected in ADR-054; proxy is the right deployment
  shape).

## Success criteria

- A worker call to `provider.complete(capability_tag="frontier-reasoning",
  …)` resolves to the model group configured in LiteLLM and returns a
  Pydantic instance conforming to `output_schema`.
- Synchronous telemetry appears in the completion event.
- LiteLLM's callback POSTs cost telemetry that pipeline-cli successfully
  reconciles against the worker-reported telemetry for the same session ID.
- Moving the LiteLLM endpoint (changing `LITELLM_BASE_URL`) requires no SDK
  or worker code changes.