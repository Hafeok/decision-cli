---
id: TC-279
title: 'agent loop: routes Scaleway endpoint via LiteLLM with model group resolution'
type: scenario
status: unimplemented
validates:
  features:
  - FT-123
  adrs:
  - ADR-069
  - ADR-064
phase: 4
observes:
- exit-code
runner: pytest
runner-args: workers/code-writer/tests/test_provider_routing.py::test_scaleway_routes_via_litellm
runner-timeout: 60
---

## Description

Scaleway is a first-class provider in this design — same as Anthropic, routed through LiteLLM's model groups, same loop code path. This TC asserts the Scaleway case exercises NO branch that the Anthropic case did not. The dispatch payload's `endpoint="scaleway"` should be a *label* on telemetry, not a fork in routing logic.

The legacy SCW_SECRET_KEY + DEC_YROUTER_URL env-var path from `env_routing.py` MUST NOT be exercised — LiteLLM owns Scaleway's auth and routing now.

## Acceptance Criteria

Pytest test at `workers/code-writer/tests/test_provider_routing.py::test_scaleway_routes_via_litellm`.

Setup:

- Env: `LITELLM_BASE_URL=http://proxy.test:4000`, `LITELLM_API_KEY=sk-virtual-001`. **No `SCW_SECRET_KEY` or `DEC_YROUTER_URL` in env.**
- `DispatchPayload` with `endpoint="scaleway"`, `model_id="scaleway-llama-3.3-70b-instruct"` (a LiteLLM model group name).
- `litellm.completion` patched; captures `kwargs`.

Assertions:

- `litellm.completion` was called with `base_url="http://proxy.test:4000"`, `api_key="sk-virtual-001"`, `model="scaleway-llama-3.3-70b-instruct"` — identical to the Anthropic case's call shape except for `model`.
- No reference to `SCW_SECRET_KEY` or `DEC_YROUTER_URL` was read during the dispatch (mock `os.environ.__getitem__` and assert these keys were never queried, OR assert the deletion of `env_routing.py` by verifying `from code_writer.env_routing import claude_env_for` raises `ImportError`).
- `response.status == "ok"`.

This is the test that locks down the "provider-agnostic from the start" property. If a future PR re-introduces a Scaleway-specific branch in the loop, this test catches it.
