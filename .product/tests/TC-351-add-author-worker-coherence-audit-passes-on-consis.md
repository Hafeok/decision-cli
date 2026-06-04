---
id: TC-351
title: add-author-worker coherence audit passes on consistent synthetic positive cluster
type: scenario
status: passing
validates:
  features:
  - FT-140
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-351-cluster-audit-author-positive.sh
runner-timeout: 60
last-run: 2026-06-04T15:47:44.739819852+00:00
last-run-duration: 0.0s
---

## Context

Positive coherence-audit TC for [FT-140](FT-140) — the `add-author-worker` cluster. Asserts the audit script `scripts/checks/cluster-audit-add-author-worker.py` passes when fed a synthetic-positive fixture in which all six cells emit a consistent contract.

## Setup

- A fixture directory under `tests/fixtures/cluster-audit-add-author-worker/positive/` containing:
  - `capability_binding/seed.nq` — well-formed role + capability + binding quads.
  - `pydantic_io_models/models.py` — declares `Input` with realistic fields (brief, gap_context, context_excerpt) and `Output` with `body_markdown: str`, `sections: dict[str, str]`, plus `EXPECTED_SECTIONS: list[str] = ["Description", "Functional Specification", "Out of scope"]`.
  - `system_prompt/system.md` — references each of `## Description`, `## Functional Specification`, `## Out of scope` headings as required output sections.
  - `agent_loop/loop.py` — calls `litellm.completion(model=payload.model_id, base_url=LITELLM_BASE_URL, ...)` matching the canonical FT-123 shape; reads only `payload` fields that exist on `Input`.
  - `fixtures_example_inputs/example_001.json`, `example_002.json` — both validate against the `Input` schema.
  - `unit_tests/test_author.py` — loads `example_001.json`, drives the loop with LiteLLM stubbed, asserts `output.body_markdown` contains each H2 in `EXPECTED_SECTIONS`.

## Steps

1. Run `scripts/checks/cluster-audit-add-author-worker.py` against the positive fixture directory, passing the six cell-output paths via the documented CLI args.
2. Capture exit code and stderr.

## Expected outcome

- Exit code 0.
- Stderr lists `PASS` for each of the six checks: `agent_loop_calls_litellm_canonical`, `output_schema_has_body_and_sections`, `system_prompt_references_h2_sections`, `fixtures_validate_against_input_schema`, `unit_tests_construct_output_through_stubbed_loop`, `output_is_draft_not_verdict`.
- No `FAIL` lines.

## Pass / fail

- Pass: bash runner returns exit 0; the audit script reports six `PASS` lines.
- Fail: any non-zero exit code or any `FAIL` line.

## Why this matters

Establishes the audit's positive contract surface — proves the six checks are runnable end-to-end on a well-formed cluster, so the negative test (TC-352) has a meaningful baseline. Without the positive case landing, the negative case cannot prove the audit's discriminatory power; both halves are needed for the audit's "teeth" property to hold.