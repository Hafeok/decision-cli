---
id: TC-274
title: 'tool_safety: tool_result_error returns the LiteLLM/OpenAI tool-result shape'
type: scenario
status: unimplemented
validates:
  features:
  - FT-124
  adrs:
  - ADR-071
phase: 4
observes:
- exit-code
runner: pytest
runner-args: workers/_shared/tests/test_tool_safety.py::test_tool_result_error_shape
runner-timeout: 30
---

## Description

`tool_result_error(tool_use_id, message)` builds the structured error block the agentic loop returns to the model when a tool refuses to execute (containment violation, secrets pattern match, etc.). The shape must match the LiteLLM/OpenAI tool-result format exactly — otherwise the model sees a malformed message and the loop drops the conversation thread.

This is a small but load-bearing contract test: the agentic loop in FT-123 will round-trip these blocks into `messages`, so the shape has to match what `litellm.completion` expects on the next turn.

## Acceptance Criteria

Pytest test at `workers/_shared/tests/test_tool_safety.py::test_tool_result_error_shape`.

Call `tool_result_error("toolu_001", "write to .env blocked: secrets pattern")` and assert the returned dict is exactly:

```python
{
    "type": "tool_result",
    "tool_use_id": "toolu_001",
    "content": [{"type": "text", "text": "write to .env blocked: secrets pattern"}],
    "is_error": True,
}
```

Additional assertions:

- The function accepts `tool_use_id` as a string and writes it verbatim — no encoding/escaping.
- The function accepts arbitrary `message` strings (including ones with newlines, unicode, quotes); the `content` block's `text` is the unaltered message.
- The returned dict is JSON-serialisable (`json.dumps(...)` round-trips). This is the property that lets the loop append the dict directly into the `messages` list passed to `litellm.completion`.
