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
- ADR-013
- ADR-016
tests:
- TC-146
domains: []
domains-acknowledged:
  ADR-021: ADR-021 governs action-interpretation agreement as a fitness metric. Not applicable without a paired action-interpretation session.
  ADR-035: ADR-035 governs Bundle.stakes as a first-class judgment field. This feature does not assemble a stakes-bearing bundle.
  ADR-044: ADR-044 governs Brief as a typed artifact in product-cli's catalog. This feature was not authored from a Brief.
  ADR-012: ADR-012 governs per-stream working-directory discovery. This feature does not introduce a stream-bound command.
  ADR-041: ADR-041 governs SHACL enforcement at the GraphWriter chokepoint. This feature does not write artifacts through GraphWriter.
  ADR-002: ADR-002 governs graph-as-state vs event-sourced semantics. This feature's scope does not change that choice.
  ADR-024: ADR-024 governs the Feedback lifecycle state machine. Not invoked here.
  ADR-065: ADR-065 governs the Dagger deferral for the worker runtime model. This feature does not depend on the runtime model.
  ADR-004: ADR-004 governs PROV-O event and session shapes. This feature does not introduce new event or session types.
  ADR-022: ADR-022 governs Feedback as a first-class flow class. This feature does not produce Feedback artifacts.
  ADR-017: ADR-017 governs action-interpretation pairing as a structural requirement. This feature does not produce an action-interpretation pair.
  ADR-064: ADR-064 governs LiteLLM as the LLM-call substrate. This feature does not call LiteLLM.
  ADR-036: ADR-036 governs the Capability and RoleBinding catalog as graph artifacts. This feature does not extend that catalog.
  ADR-025: ADR-025 governs blocking vs non-blocking Feedback semantics. Not invoked here.
  ADR-043: ADR-043 governs full-chain traversal as a QueryTemplate artifact. This feature does not introduce a new full-chain query.
  ADR-001: ADR-001 governs the oxi-events crate's SDP boundary. This feature does not modify oxi-events' public surface.
  ADR-034: ADR-034 governs tiered escalation policy with controlled trigger vocabulary. This feature does not invoke escalation.
  ADR-005: ADR-005 governs value-stream-resident scope. This feature is not value-stream-scoped.
  ADR-027: ADR-027 governs authority declarations in the role catalog. This feature does not register a new role.
  ADR-038: ADR-038 governs dual-provenance discipline (mechanical + motivational). This feature does not introduce a new artifact type subject to dual provenance.
  ADR-040: ADR-040 governs the BoundaryArtifact class. This feature does not introduce a new boundary artifact.
  ADR-018: ADR-018 governs the VerificationVerdict schema. This feature does not produce a verification verdict.
  ADR-037: ADR-037 governs Scaleway/Anthropic provider defaults. This feature does not configure provider routing.
  ADR-033: ADR-033 governs capability-based model routing as a graph-resident layer. This feature does not route models.
  ADR-039: ADR-039 governs motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom. This feature does not introduce new motivational predicates.
  ADR-014: ADR-014 governs Architectural Fitness Functions as product-cli artifacts. This feature does not introduce a new fitness function.
  ADR-055: ADR-055 governs WorkerImage as a catalog mirroring the Model catalog. This feature does not extend that catalog.
  ADR-023: ADR-023 governs the Feedback controlled vocabulary. Not invoked here.
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