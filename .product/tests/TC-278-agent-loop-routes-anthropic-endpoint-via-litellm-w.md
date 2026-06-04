---
id: TC-278
title: 'agent loop: routes Anthropic endpoint via LiteLLM with model group resolution'
type: scenario
status: passing
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
runner-args: workers/code-writer/tests/test_provider_routing.py::test_anthropic_routes_via_litellm
runner-timeout: 60
last-run: 2026-06-04T12:26:02.266912072+00:00
last-run-duration: 0.4s
---

## Description

The Anthropic-endpoint pin in the dispatch payload (per FT-066 / [ADR-033](ADR-033)) is preserved on the wire but the loop NEVER calls Anthropic's SDK directly. Every call goes through `litellm.completion` with `base_url=LITELLM_BASE_URL` and `api_key=LITELLM_API_KEY`. This TC asserts that contract for the Anthropic case.

## Acceptance Criteria

Pytest test at `workers/code-writer/tests/test_provider_routing.py::test_anthropic_routes_via_litellm`.

Setup:

- Env: `LITELLM_BASE_URL=http://proxy.test:4000`, `LITELLM_API_KEY=sk-virtual-001`.
- `DispatchPayload` with `endpoint="anthropic"`, `model_id="anthropic-claude-sonnet-4-5"` (a LiteLLM model group name, not a raw Anthropic identifier).
- `litellm.completion` patched. The patch captures the `kwargs` it receives.

Assertions:

- The patched `litellm.completion` was called with `base_url="http://proxy.test:4000"`, `api_key="sk-virtual-001"`, `model="anthropic-claude-sonnet-4-5"`.
- The worker did NOT import or invoke the `anthropic` SDK — assert `"anthropic" not in sys.modules` (or, if `anthropic` is transitively imported by `litellm`, assert no top-level worker code references it via `grep` in a separate static check).
- The Anthropic-specific `extra_body` (if any) is passed through unmolested for things like cache_control; the test asserts the absence of `extra_body` when the dispatch has no Anthropic-specific knobs.