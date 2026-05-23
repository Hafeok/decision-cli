---
id: TC-105
title: Scaleway client wrapper builds with SCW_SECRET_KEY and surfaces missing-key error
type: exit-criteria
status: passing
validates:
  features:
  - FT-059
  adrs: []
phase: 2
runner: pytest
runner-args: workers/_shared/tests/test_scaleway_client.py
runner-timeout: 120
last-run: 2026-05-23T18:00:11.431804643+00:00
last-run-duration: 0.4s
---

## Description

Scenario: the Scaleway client wrapper at `workers/_shared/src/_shared/scaleway_client.py` ([FT-059](FT-059)) builds against `SCW_SECRET_KEY`, surfaces a structured error when the key is missing, and completes a smoke chat-completions call against `qwen3-coder-30b-a3b-instruct` when the key is present.

The runner is `pytest`. Two test modes:

- **CI mode (default, no live credentials).** Uses a fake `OpenAI` class via monkeypatching to assert client construction, request shape, and error handling.
- **Live mode (skipped unless `SCW_SECRET_KEY` is set and `SCALEWAY_LIVE=1`).** Issues a real chat-completions call to verify wire compatibility.

Acceptance:

1. **`build_client` constructs with key.** Set `SCW_SECRET_KEY=test-key`. Call `build_client()`. Assert the returned `OpenAI` instance has `base_url = "https://api.scaleway.ai/v1"` and `api_key = "test-key"`.
2. **`build_client` raises on missing key.** Unset `SCW_SECRET_KEY`. Call `build_client()`. Assert it raises `ScalewayClientError` with a message that names `SCW_SECRET_KEY` and hints at rebinding to `endpoint=anthropic`.
3. **`missing_key_error_or_none` is non-raising.** Unset the key. Call `missing_key_error_or_none()`. Assert it returns a `ScalewayClientError` instance (does not raise). Set the key; call it; assert it returns `None`.
4. **`scaleway_chat_caller` request shape.** Build a fake client whose `.chat.completions.create` records its kwargs. Call the caller with `(system="sys", user="user", model_id="qwen3-coder-30b-a3b-instruct", max_tokens=128)`. Assert the recorded call has `model="qwen3-coder-30b-a3b-instruct"`, `max_tokens=128`, messages=[{role:"system",content:"sys"},{role:"user",content:"user"}].
5. **Token extraction.** Fake response with `usage.prompt_tokens=10`, `usage.completion_tokens=20`. Assert the returned tuple is `("<content>", 10, 20)`.
6. **No key logging.** Assert that under `caplog.set_level(DEBUG)`, no log record contains the substring `test-key`.
7. **Live smoke (opt-in).** When `SCALEWAY_LIVE=1`, issue a real call with model `qwen3-coder-30b-a3b-instruct`, system="reply with the single word OK", user="hi", max_tokens=10. Assert the response text contains "OK" (case-insensitive) and tokens are positive integers.

⟦Σ:Types⟧{
  ClientResult ≜ ⟨text:String, tokensIn:Nat, tokensOut:Nat⟩
}

⟦Γ:Invariants⟧{
  build_client() raises ScalewayClientError ⇔ SCW_SECRET_KEY ∉ env
  ∀ log_record: secret_key_value ∉ log_record.message
}