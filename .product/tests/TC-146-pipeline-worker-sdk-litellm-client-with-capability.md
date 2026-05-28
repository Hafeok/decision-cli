---
id: TC-146
title: 'pipeline-worker SDK: LiteLLM client with capability-tag dispatch and structured output — exit criterion'
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-146-pipeline-worker-sdk-provider.sh
runner-timeout: 180
last-run: 2026-05-28T08:48:32.051861798+00:00
last-run-duration: 4.0s
---

## Description

Exit criterion for [FT-081](FT-081). Proves the four facts the parent
feature_spec names:

1. **Capability-tag round-trip.** A worker call to
   `provider.complete(capability_tag="frontier-reasoning", …,
   output_schema=SomePydanticModel)` resolves the tag to a LiteLLM model
   group (workers never see the model name) and returns a Pydantic
   instance conforming to `output_schema`.
2. **Synchronous telemetry.** The result carries a `CompletionTelemetry`
   block with tokens, model, provider, latency, and retry count — the
   worker-side authoritative provenance per [ADR-054](ADR-054)
   §"competing source-of-truth concern". The telemetry merges cleanly
   into a `Session` so it rides through to the completion payload the
   wire layer POSTs back.
3. **DDD session id propagation.** `metadata={"ddd_session_id": …}` is
   forwarded verbatim to LiteLLM so its async logging callback can
   correlate the cost record with the worker-reported telemetry on the
   harness side.
4. **Configurable endpoint.** `LITELLM_BASE_URL` and `LITELLM_API_KEY`
   are read at `LiteLLMConfig.from_env(...)` construction; moving the
   proxy is an env-var change, not a code change ([ADR-053](ADR-053)).

The test does not require a live LiteLLM proxy — both `LiteLLMClient`
and `Provider` accept an injectable completion function so the SDK code
paths under test (capability-tag forwarding, telemetry extraction,
metadata threading, structured-output coercion) are exercised end-to-end
without network I/O. Production deployments wire the real
`litellm.acompletion` and `instructor.from_litellm(...)` in via the
same injection points.

Twelve test cases cover the four success criteria plus the structural
constraints (no per-provider methods on the public surface per
[ADR-054](ADR-054); validation-failure path raises a typed
`ValidationError`; passthrough `extra_body` for provider-specific
parameters reaches LiteLLM untouched).