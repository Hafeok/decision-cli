---
id: FT-096
title: 'worker-distribution: LiteLLM proxy deployment as the LLM-call substrate'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-064
- ADR-013
- ADR-016
- ADR-044
- ADR-047
- ADR-054
tests:
- TC-138
domains: []
domains-acknowledged:
  ADR-039: No new motivational predicate introduced by this feature.
  ADR-040: No new boundary artifact introduced by this feature.
  ADR-055: Cross-cutting ADR reviewed; not in this slice-1 worker-distribution feature's scope. Brief-internal governance is captured under ADR-055..ADR-065 and linked where applicable.
  ADR-021: Feature does not produce an action-interpretation pair, so the agreement metric does not apply.
  ADR-038: No new artifact type introduced by this feature; existing dual-provenance discipline already governs the artifacts written elsewhere in the brief.
  ADR-024: No Feedback artifact produced; lifecycle state machine not invoked here.
  ADR-002: Feature ships infrastructure / packaging conventions, not graph mutations.
  ADR-017: Feature is not an action-interpretation pair; no paired interpretation session involved.
  ADR-004: Feature does not emit dispatch or session events; PROV-O hookup happens in features that write artifacts.
  ADR-036: WorkerImage catalog (ADR-055) mirrors the Capability/RoleBinding catalog shape, but this feature does not extend the Capability/RoleBinding catalog itself.
  ADR-001: Application-layer feature; does not touch the oxi-events crate boundary.
  ADR-025: No Feedback artifact produced; blocking semantics not invoked here.
  ADR-033: Worker SDK provider routing is governed by ADR-047 (capability-tag binding) and ADR-054 (LiteLLM as substrate); ADR-033's earlier formulation does not apply.
  ADR-034: Worker registration flow does not invoke escalation tiers; the WorkerCurator's reject path produces Feedback, not an escalation step.
  ADR-023: No Feedback artifact produced; controlled vocabulary not invoked here.
  ADR-012: Not a per-stream command; no working-directory walk-up involved.
  ADR-022: No Feedback artifact produced by this feature.
  ADR-043: Feature does not introduce a new full-chain query; existing traversal already covers the artifacts it produces.
  ADR-027: No new role registered by this feature.
  ADR-035: Feature does not assemble a bundle that carries a stakes judgment.
  ADR-005: Worker-registration discipline is independent of value-stream scope.
  ADR-014: No new fitness function introduced; cost-reconciliation drift (ADR-064) and action-interpretation agreement (ADR-021) cover the slice-1 worker fitness signals.
  ADR-037: Provider defaults (Scaleway / Anthropic) are configured inside LiteLLM (ADR-064) rather than in feature code.
  ADR-018: No verification verdict artifact produced by this feature.
  ADR-065: Dagger deferral is a runtime-substrate decision affecting FT-088 / FT-095; this feature does not depend on the runtime model.
  ADR-041: Feature does not write artifacts through GraphWriter; SHACL enforcement is in scope of FT-086 / FT-087 / FT-094.
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
