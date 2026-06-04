---
id: TC-281
title: 'agent loop: FT-108 addressed feedback extraction still works post-migration'
type: scenario
status: passing
validates:
  features:
  - FT-123
  - FT-108
adrs:
  - ADR-069
phase: 4
observes:
- exit-code
runner: pytest
runner-args: workers/code-writer/tests/test_addressed_feedback_extraction.py::test_extracts_citations_from_litellm_response
runner-timeout: 60
last-run: 2026-06-04T12:26:02.266912072+00:00
last-run-duration: 0.3s
---

## Description

FT-108 added a citation-block extractor that reads the final assistant message to find which feedback artifacts the model claimed to have addressed. The extractor lived inside `_subprocess_runner.py`'s stream-json scraping code; FT-123 moves it into `agent/responses.py`. This TC asserts the extractor's behaviour is unchanged across the migration — same input → same `addressed_feedback` output on `WorkerResponse`.

## Acceptance Criteria

Pytest test at `workers/code-writer/tests/test_addressed_feedback_extraction.py::test_extracts_citations_from_litellm_response`.

Setup:

- `DispatchPayload` with `allowed_tools=["read_file", "write_file"]`.
- The bundle in `payload` references two open feedback artifacts: `fb_001` (DEFECT) and `fb_002` (GAP).
- `litellm.completion` is patched. The final response's assistant text contains: `"Addressed fb_001 by updating the import. fb_002 is out of scope for this dispatch."`
- `stop_reason="end_turn"`.

Assertions:

- `response.addressed_feedback` is a list containing exactly `"fb_001"` (the citation extractor recognises only feedback IDs explicitly addressed, not mere mentions).
- `"fb_002"` is NOT in `addressed_feedback` (mention != address).
- The extractor reads from the **final assistant message text**, not from intermediate `tool_use` turns — this assertion was the regression FT-108 originally guarded against.

This test pins the FT-108 contract across the migration. If the citation extractor moves but loses its semantics, this fails.