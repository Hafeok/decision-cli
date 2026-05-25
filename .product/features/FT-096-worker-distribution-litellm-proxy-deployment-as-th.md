---
id: FT-096
title: 'worker-distribution: LiteLLM proxy deployment as the LLM-call substrate'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-064
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. Workers route every LLM call through a LiteLLM proxy that holds provider API keys; workers themselves carry only a scoped virtual key. This is the deployment side of ADR-064 (LiteLLM as substrate); the worker-SDK consumer side lives in ADR-054 / FT-081.

## Scope

- LiteLLM proxy deployment shape (slice 1, single-tenant, on operator's machine):
  - Runs locally (or sidecar container) on a known port — default `localhost:4000`.
  - Configured via `config.yaml` declaring model groups and their backing providers, e.g.:
    ```yaml
    model_list:
      - model_name: frontier-reasoning
        litellm_params:
          model: anthropic/claude-opus-4-5
          api_key: os.environ/ANTHROPIC_API_KEY
      - model_name: fast-cheap
        litellm_params:
          model: anthropic/claude-haiku-4-5
          api_key: os.environ/ANTHROPIC_API_KEY

    general_settings:
      master_key: os.environ/LITELLM_MASTER_KEY
      database_url: os.environ/LITELLM_DB_URL   # optional in slice 1

    litellm_settings:
      callbacks: ["pipeline-cli-telemetry"]
    ```
  - `model_name` values are the framework's capability tags. Workers calling LiteLLM with `model="frontier-reasoning"` get routed to whatever provider + model that group is bound to. New providers / models / fallbacks land as config edits, not code changes.
- Virtual key issuance:
  - At proxy startup, issue at least one virtual key via LiteLLM's `/key/generate`, scoped to the configured model groups, with a budget appropriate for slice 1 (low; local dev).
  - The key is written into the operator's `workers.env` (or equivalent) so `pipeline-cli workers run` (FT-095) injects it into worker containers as `LITELLM_API_KEY`.
- Telemetry callback (`pipeline-cli-telemetry`, implemented as a custom LiteLLM callback class):
  - POSTs every call's telemetry — tokens, latency, cost, model, provider, fallback chain, retry count — to pipeline-cli's `/llm-call-telemetry` reconciliation endpoint.
  - Indexed by the `ddd_session_id` metadata that workers propagate (per `pipeline-worker-sdk`'s provider-abstraction feature).
- Slice 1 ships at least one model group: Anthropic via Anthropic's API.

## Out of scope

- Persistent spend-tracking DB (slice 2 progression).
- Per-WorkerImage virtual keys (slice 2 progression; slice 1 uses a shared key per operator).
- Multi-tenant LiteLLM (`feature:multi-tenant-litellm`, slice 3+).
- HSM-backed master key custody (slice 2+).
- Building our own LLM proxy from scratch (explicitly rejected per ADR-064).

## Success criteria

- A worker container running per FT-095 calls `LITELLM_BASE_URL` with `model="frontier-reasoning"` and receives a completion routed through Anthropic.
- The proxy's `pipeline-cli-telemetry` callback successfully POSTs telemetry to pipeline-cli; the session record can be queried for that call's cost figure (sourced from LiteLLM, authoritative for spend per ADR-064).
- Adding a second model group (e.g. Scaleway or OpenAI) requires only a `config.yaml` edit + proxy restart; no SDK or worker code change.
- Provider API keys (e.g. `ANTHROPIC_API_KEY`) appear nowhere in worker container env — only in the LiteLLM process's env.
