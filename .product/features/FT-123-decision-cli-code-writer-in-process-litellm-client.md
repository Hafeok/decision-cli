---
id: FT-123
title: 'decision-cli: code-writer in-process LiteLLM-client agentic loop (retire claude -p subprocess)'
phase: 4
status: planned
depends-on:
- FT-122
- FT-124
- FT-066
adrs:
- ADR-069
- ADR-008
- ADR-054
- ADR-064
- ADR-070
- ADR-071
tests:
- TC-275
- TC-276
- TC-277
- TC-278
- TC-279
- TC-280
- TC-281
- TC-282
domains:
- api
- security
domains-acknowledged:
  api: "Replaces the `claude -p` subprocess in `workers/code-writer/` with an in-process Python agentic loop using `litellm.completion`. Five tool primitives exposed to the model via the OpenAI-shaped function-calling protocol. Provider-agnostic via LiteLLM (Anthropic + Scaleway first-class)."
  security: "Tool surface is enforced by intersection with `payload.allowed_tools` ([ADR-070](ADR-070)); empty intersection fails closed. Every tool routes through `_shared.tool_safety` for workspace containment and secrets blocking ([ADR-071](ADR-071))."
---

## Description

This is the central feature of the tool-scoped worker port. After it lands, `workers/code-writer/` no longer spawns `claude -p`; it runs an in-process Python loop that calls `litellm.completion` directly, walks every `tool_use` block the model returns, dispatches to one of five registered tool primitives (`read_file`, `write_file`, `run_build`, `run_lint`, `run_tests`), threads the `tool_result` blocks back into the next turn, and stops on `end_turn`.

Provider-agnostic from day one. The substrate is LiteLLM per [ADR-054](ADR-054)/[ADR-064](ADR-064); Anthropic and Scaleway are both first-class — they are model groups in the LiteLLM proxy config, not branches in worker code. Adding a third provider is a LiteLLM config edit, not a worker release.

The tool surface enforced by the loop is the intersection of `payload.allowed_tools` (delivered by FT-122) with the worker's tool registry. Empty intersection → fail-closed `WorkerError(category="invalid_dispatch")` before the first LLM call. Every tool routes through `_shared.tool_safety` (FT-124) for path containment and secrets blocking.

`_subprocess_runner.py` is deleted in the same commit; `env_routing.py` shrinks (no more env-var-based provider routing — LiteLLM owns that now). The worker is reinstalled via `uv tool install workers/code-writer --reinstall` after the change.

## Functional Specification

### Inputs

No new operator-facing CLI surface. The dispatch payload from FT-122 is the input contract. Two new env vars consumed (transparently — these are already in the deployment story per [ADR-053](ADR-053)):

- `LITELLM_BASE_URL` (default `http://localhost:4000`) — LiteLLM proxy address.
- `LITELLM_API_KEY` — LiteLLM virtual key. The worker holds only this; provider keys live in the LiteLLM config.

### Outputs

New Python package layout under `workers/code-writer/src/code_writer/agent/`:

- `agent/__init__.py` — public `run_agent(payload: DispatchPayload) -> WorkerResponse`.
- `agent/loop.py` — the `litellm.completion` poll loop with tool-use cycling.
- `agent/tools.py` — `TOOL_REGISTRY` dict mapping snake_case name to `(input_schema, dispatcher_fn)`. Five entries: `read_file`, `write_file`, `run_build`, `run_lint`, `run_tests`.
- `agent/prompts.py` — system prompt + FT-108 feedback prefix logic (moved out of `_subprocess_runner._write_bundle_prompt`).
- `agent/responses.py` — `_build_success_response`, `_no_tools_response`, `_max_turns_response`, `_timeout_response` builders (moved out of `_subprocess_runner`).

Deleted:

- `workers/code-writer/src/code_writer/_subprocess_runner.py` (502 LoC, replaced by the package above).
- The Scaleway env-var branch in `env_routing.py` (collapses to a single `litellm` client construction helper; the file may be deleted entirely if no other consumer remains).

Updated:

- `workers/code-writer/src/code_writer/claude_runner.py` (or its equivalent dispatch entry) calls `from .agent import run_agent` instead of `from ._subprocess_runner import run_claude`.
- `workers/code-writer/pyproject.toml` adds `litellm>=1.50` as a dependency. Removes any `anthropic` direct dependency if present (LiteLLM owns provider SDKs).

### Behaviour

1. **Entry.** `run_agent(payload)` validates `payload.workspace_path` exists (mkdir if not), intersects `payload.allowed_tools` with the keys of `TOOL_REGISTRY`. Empty → `WorkerResponse(status="error", error.category="invalid_dispatch", error.message="no tools granted")`, no LLM call.
2. **System prompt.** Composed from the bundle in `payload` via `prompts.render_system_prompt(payload)` — preserves the FT-108 addressed-feedback prefix logic verbatim.
3. **First message.** A `user` role message containing the bundle's instruction body.
4. **Loop.** For `turn in range(payload.max_turns)`:
   - Call `litellm.completion(model=payload.model_id, messages=messages, tools=tool_schemas, base_url=LITELLM_BASE_URL, api_key=LITELLM_API_KEY)`. Use `extra_body={...}` for any Anthropic-specific knobs per [ADR-064](ADR-064).
   - Observe the response: record the assistant turn in telemetry; if `stop_reason in ("end_turn", "stop")`, extract the final text and call `_build_success_response`.
   - Otherwise (`stop_reason == "tool_use"` or OpenAI-shaped `tool_calls`): for each tool call, look up the dispatcher in `TOOL_REGISTRY`, execute via the dispatcher (which uses `_shared.tool_safety.safe_join` and `is_write_blocked`), build a `tool_result` content block via `_shared.tool_safety.tool_result_error` (on error) or a plain `tool_result` (on success), append assistant message + tool_result message to `messages`, loop.
5. **Max turns exhausted.** `_max_turns_response(payload, tool_calls, started_at)` returns `WorkerResponse(status="error", error.category="max_turns_exceeded")`. Telemetry preserves the partial tool-call history.
6. **Auth & routing.** Always go through LiteLLM. There is no branch on `payload.endpoint` in the loop code — LiteLLM's model groups encode the endpoint. The `payload.endpoint` field is preserved on the wire (FT-066 / [ADR-033](ADR-033)) for telemetry and audit; the worker reads `payload.model_id` for the `model=` argument.
7. **No subprocess.** The loop MUST NOT call `subprocess.run`, `subprocess.Popen`, `os.system`, `os.exec*`, or any equivalent for invoking models. `claude` and `anthropic-cli` binaries are forbidden. (The `run_*` tool primitives DO call subprocess — but for project build/lint/test commands per [ADR-071](ADR-071), not models. See FT-123 boundary tests below.)

### Tool primitives (concrete schemas)

```python
TOOL_SCHEMAS = {
    "read_file": {
        "name": "read_file",
        "description": "Read a file in the workspace. Returns content with 1-indexed line numbers.",
        "input_schema": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Workspace-relative path."},
                "start_line": {"type": "integer", "minimum": 1},
                "end_line": {"type": "integer", "minimum": 1},
            },
            "required": ["path"],
        },
    },
    "write_file": {
        "name": "write_file",
        "description": "Write a file, or replace an exact string. Use old_string=null for full-file overwrite.",
        "input_schema": {
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "content": {"type": "string"},
                "old_string": {"type": ["string", "null"]},
            },
            "required": ["path", "content"],
        },
    },
    "run_build": {
        "name": "run_build",
        "description": "Run the project build. Returns combined stdout/stderr + exit code.",
        "input_schema": {
            "type": "object",
            "properties": {"target": {"type": "string", "enum": ["rust", "python", "all"], "default": "all"}},
        },
    },
    "run_lint": {
        "name": "run_lint",
        "description": "Run the project linter. Returns diagnostics.",
        "input_schema": {
            "type": "object",
            "properties": {"scope": {"type": "string", "enum": ["rust", "python", "all"], "default": "all"}},
        },
    },
    "run_tests": {
        "name": "run_tests",
        "description": "Run the project test suite. Optional filter.",
        "input_schema": {
            "type": "object",
            "properties": {
                "scope": {"type": "string", "enum": ["rust", "python", "all"], "default": "all"},
                "filter": {"type": "string", "description": "Test-name filter (cargo test / pytest -k)."},
            },
        },
    },
}
```

Each dispatcher:

- Resolves paths via `_shared.tool_safety.safe_join` and returns `tool_result_error` on `WorkspaceViolation`.
- Writes go through `is_write_blocked` first.
- `run_*` execute with `cwd=workspace_path`, `timeout=min(120, payload.timeout_seconds // 4)`, `capture_output=True`, `text=True`. Stdout+stderr concatenated, truncated at `MAX_TOOL_OUTPUT_BYTES` per [ADR-071](ADR-071).

### Acceptance criteria

- Given an Anthropic-routed Claude Sonnet 4.5 model group in LiteLLM, a dispatch with `allowed_tools=["read_file","write_file"]`, and a bundle asking to create a simple file: the loop completes, the file is created in the workspace, `WorkerResponse.status="ok"`, and at least one `ToolCall(name="write_file", status="ok")` appears in telemetry.
- Given a Scaleway-routed model group in LiteLLM with the same dispatch: the loop completes successfully against the Scaleway endpoint with the same `WorkerResponse` shape. No code path differs from the Anthropic case.
- Given `allowed_tools=[]`: the loop returns `WorkerResponse(status="error", category="invalid_dispatch")` before any `litellm.completion` call. (TC-271, owned by FT-122 but observed here.)
- Given a 1-turn `max_turns` cap and a bundle that needs 3 turns: the loop returns `WorkerResponse(status="error", category="max_turns_exceeded")` with the partial tool-call telemetry preserved.
- Pytest patches `subprocess.Popen` and `subprocess.run` to raise — the dispatch completes successfully (proves no model-invoking subprocess fires). The `run_*` tools' subprocess calls happen ONLY when `allowed_tools` includes them AND the model invokes them; the tests for those use the actual subprocess against a stub workspace.
- FT-108 addressed-feedback extraction still works: the loop captures the final assistant message text; `_extract_addressed_feedback` runs as before.
- Source-file budget per [ADR-013](ADR-013): every new file under `workers/code-writer/src/code_writer/agent/` is ≤ 400 lines.

### Non-goals

- Replacing the worker SDK / claim-pull architecture. Code-writer is `run-once` per [ADR-008](ADR-008); switching to the `pipeline-worker-sdk` claim model is out of scope (rejected in [ADR-069](ADR-069) alternatives).
- LiteLLM proxy setup or config in this feature. We consume the proxy at `LITELLM_BASE_URL`; setting it up is [FT-096](FT-096)'s job. Operators running this feature against a stand-alone Anthropic API key without a LiteLLM proxy will hit `litellm`'s own routing — fine for dev, not in scope for verification here.
- Audit-quad writes to the session graph. Owned by FT-125. This feature delivers the in-memory `ToolCall` telemetry on `WorkerResponse`; FT-125 persists it as `dec:ToolCall` quads.
- Retry orchestration. Owned by [FT-104](FT-104). The loop returns terminal `WorkerResponse`s; the harness decides retry.

## Migration / rollout

**Hard cutover.** No env-flag coexistence path. `_subprocess_runner.py` is deleted in the same commit. The `CODE_WRITER_STUB=1` switch for tests remains untouched (it has its own fork in `claude_runner.py`).

**Operator runbook addendum.** After implementation:

```bash
uv tool install workers/code-writer --reinstall
```

The worker is on `$PATH` as `code-writer` (or whatever the existing entry-point binds). `dec implement` invokes the binary, not the source — without reinstall, dispatches still hit the old `claude -p` path. This step lands in `decision-cli-slice-1-bounds.md` or in a dedicated "post-FT-123 reinstall" note.

## Exit Criteria (Test Coverage)

Per [ADR-013](ADR-013), behaviours above are asserted by TCs linked to this feature.
