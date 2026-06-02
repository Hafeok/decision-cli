---
id: ADR-069
title: In-process LiteLLM-client agentic loop for code-writer (retire claude -p subprocess)
status: accepted
features: []
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:0c92716493677952253c4fad0170af6c2e88364aa41b0d385c474a2cca388abf
---

## Context

The code-writer worker (`workers/code-writer/`) is the implementer for `dec implement`. Today it runs the implementer role by spawning `claude -p --dangerously-skip-permissions` as a subprocess from `_subprocess_runner.py:281-320, 313` and scraping its stream-json stdout. That works, but it has four problems that compound as the orchestrator gets more capable:

1. **Tool surface is whatever `claude -p` defaults to.** Today the worker passes no `--allowed-tools` flag; the Claude Code subprocess uses its full built-in tool surface (Read, Write, Edit, Glob, Grep, Bash, etc.). The `DispatchPayload.allowed_tools` field (`models.py:87`) exists but is read by no code. Tool scoping per role — implementer-vs-verifier-vs-reviewer — is impossible without an external control plane.
2. **Provider routing is bolted on.** Scaleway routing goes through env vars (`SCW_SECRET_KEY`, `DEC_YROUTER_URL`) feeding `env_routing.claude_env_for()` (`env_routing.py:56-91`), which the subprocess inherits. That works for two endpoints but doesn't generalise. The rest of the workspace already committed to LiteLLM as the provider substrate ([ADR-054](ADR-054), [ADR-064](ADR-064)); code-writer is the outlier.
3. **No tool-call audit.** The subprocess emits `tool_use` blocks in its stream-json output but the worker only counts file writes (`_subprocess_runner.py:60-66`). Which tool ran with which arguments and what it returned never reaches the session graph. This blocks the audit fitness signals we want for [ADR-050](ADR-050)-style provenance, and it blocks per-role tool-usage analytics.
4. **Retry semantics belong to Claude Code, not us.** Slice 1 retries by re-running `claude -p --resume {sessionId}`; we don't own the loop, so we can't decide whether a tool failure is recoverable, whether to switch models on the second try, or whether to back off. [FT-104](FT-104) wants retry orchestration in the harness — that's incompatible with delegating the loop to `claude -p`.

The pipeline factory (`/home/hafeok/projects/pipeline`) solves (1) and (3) by generating a per-step `.mcp.json` plus a JWT step token with an `allowed_servers` claim, and by routing every tool through an MCP server that audits to JSONL. We borrow the *principle* — every dispatch declares its allowed tool surface, the worker enforces it, every call is audited — without the JWT/MCP-server-per-step deployment cost. `dec` is in-process; we don't need a control plane to enforce what a single Python loop can enforce itself.

[ADR-054](ADR-054) and [ADR-064](ADR-064) already settled the provider question for the worker SDK: LiteLLM is the substrate, OpenAI-shaped API at the worker layer, provider-specific features pass through via `extra_body`. The sibling `pipeline-worker-sdk` package already uses `litellm.acompletion` (`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/provider/litellm_client.py:96-250`). Code-writer is the only worker still talking to a vendor SDK indirectly via subprocess.

## Decision

Replace the `claude -p` subprocess in `workers/code-writer/` with an **in-process LiteLLM-client agentic loop** that owns the tool-use cycle end-to-end.

Concrete substance:

1. **Provider substrate is `litellm.completion`**, in-process. Same library, same routing semantics as [ADR-054](ADR-054). `LITELLM_BASE_URL` and `LITELLM_API_KEY` ([ADR-053](ADR-053)) carry the deployment topology; the worker holds no provider keys. Anthropic and Scaleway are both first-class — they are model groups in the LiteLLM config, not branches in worker code. Adding a third provider is a LiteLLM config edit per [ADR-064](ADR-064).
2. **The loop is in-process Python.** A single function `run_agent(payload) -> WorkerResponse` polls `litellm.completion`, walks every returned `tool_use` block, dispatches to a registered Python tool, threads the `tool_result` blocks back into the next turn, and stops on `end_turn`. No subprocess, no stream-json scraping, no `--resume` token. Retry semantics belong to the harness ([FT-104](FT-104)).
3. **Tool surface is declared by the role and enforced by the worker.** The dispatch payload extends with `allowed_tools: list[str]` (the field exists today as a placeholder; this ADR makes it load-bearing). The role catalog ([ADR-070](ADR-070), separately authored) is the source of truth. The worker intersects `payload.allowed_tools` with its registry of tool primitives and refuses to start if the intersection is empty (fail-closed).
4. **Five tool primitives**: `read_file`, `write_file`, `run_build`, `run_lint`, `run_tests`. Implementer roles get all five; verifier roles drop `write_file`. Path containment and secrets blocking are enforced by [ADR-071](ADR-071) ([adr-workspace-containment-secrets-blocking](ADR-071)). Tool input/output schemas live in `agent/tools.py`; the loop is provider-agnostic and sees only the OpenAI-shaped function-calling protocol (LiteLLM translates per-provider).
5. **Every tool call is recorded** as a structured `ToolCall` on the existing `WorkerTelemetry`, and persisted as `dec:ToolCall` quads in the session named graph (deferred to a follow-up feature). Audit lives in the graph, not in JSONL files.

The dispatch payload thus grows from `(bundle, endpoint, model_identifier, parameters, capability_ref, binding_ref)` to `(bundle, endpoint, model_identifier, parameters, capability_ref, binding_ref, allowed_tools)`. The worker contract ([ADR-008](ADR-008)) absorbs this via amendment — bundle-completeness, statelessness, and no-graph-access invariants are unchanged.

## Consequences

**Positive:**

- One provider substrate across `workers/code-writer/` and `workers/pipeline-worker-sdk/`. Adding a third worker that calls models reuses the LiteLLM-client pattern by default.
- Per-role tool surfaces become declarable. A reviewer role with no `write_file` is a catalog edit, not a Python `if role == "reviewer"`.
- Tool-call audit becomes structurally possible: the loop has every `tool_use` block in hand, and `WorkerResponse` is the chokepoint that already lands in the session graph via `lifecycle.rs::assemble_implement_outcome`.
- Retry, escalation, and provider fallback move to where they belong — the harness ([FT-104](FT-104)) and LiteLLM's fallback config ([ADR-064](ADR-064)) — instead of being entangled with Claude Code's `--resume` semantics.
- Loss of the `claude -p` dependency removes one large binary from the worker's runtime footprint. The Python loop is ~250 LoC; the subprocess runner is 502.

**Negative / accepted costs:**

- We lose the `--resume {sessionId}` retry token. Mitigation: retries are fresh sessions composed by the harness; `dec implement --retry` already exists ([FT-104](FT-104)). Conversation continuity within a single dispatch is preserved by the loop itself.
- We lose Claude Code's curated tool ergonomics (auto-edit, smart Grep, Glob globbing, etc.). Mitigation: `read_file` + `write_file` + `Bash`-style `run_*` cover the implementer's needs, and the loop can call `litellm.completion` against tool-capable models like Claude Sonnet/Opus or GPT-4 indifferently. We are trading Claude Code's batteries-included for portability.
- LiteLLM's OpenAI-shaped tool API is slightly lossier than Anthropic's native shape (e.g. no cache_control on tool definitions). Mitigation: pass-through via `extra_body` is documented for Anthropic-specific knobs ([ADR-064](ADR-064)); the loop uses it where it matters.
- Increased blast radius for tool-implementation bugs: if `write_file` is wrong, the model writes wrongly. Mitigation: every tool routes through `agent/safety.py` (workspace containment + secrets blocking, [ADR-071](ADR-071)); a structured `tool_result` error round-trips to the model rather than aborting the loop.

**Boundary enforcement:**

- The worker imports `litellm`. It MUST NOT import `anthropic` or any other vendor SDK directly. This keeps the substrate decision honest.
- The loop MUST NOT spawn subprocesses to invoke models. `claude`, `anthropic-cli`, and similar binaries are forbidden imports/invocations. A regression here is the smell that motivated this ADR.
- The agent loop never reads from or writes to the orchestration graph. Bundle-in / artifact-out from [ADR-008](ADR-008) is preserved — tool calls operate on the workspace filesystem; audit quads are returned as part of `WorkerResponse` and the harness persists them.

## Alternatives considered

- **Keep `claude -p` and pass `--allowed-tools` per dispatch.** Cheapest fix for tool scoping. Rejected: leaves provider routing as the existing y-router/env-var bolt-on, perpetuates the workspace inconsistency with [ADR-054](ADR-054)/[ADR-064](ADR-064), and keeps retry / audit on the wrong side of the subprocess boundary. We solve scoping but inherit the other three problems indefinitely.
- **Anthropic Python SDK directly + y-router for Scaleway.** Was the original framing of this ADR. Rejected once [ADR-054](ADR-054)/[ADR-064](ADR-064) were factored in: vendor-specific substrate works for two providers but doesn't generalise, and the workspace already paid for LiteLLM. Switching code-writer to Anthropic SDK means writing per-provider adapters the rest of the workspace stopped owning.
- **Adopt the `pipeline-worker-sdk` package wholesale.** That SDK implements a claim-pull worker model — workers subscribe to a claim queue and post completions. Code-writer is `run-once` bundle-in/artifact-out per [ADR-008](ADR-008); adopting the SDK would re-architect the dispatch model. Rejected for slice 1; reusing the SDK's `provider/litellm_client.py` pattern (without the session/claim machinery) is the closer fit.
- **Use `claude-agent-sdk` (Anthropic's official agentic SDK) instead of rolling the loop.** Couples the substrate to one vendor's framing of agentic loops and re-introduces the per-provider lock-in we just rejected. The loop we need is ~200 lines around `litellm.completion`; the SDK saves writing those at the cost of breaking LiteLLM-substrate uniformity.
- **Wait for [FT-104](FT-104) and combine.** [FT-104](FT-104) is about retry orchestration. Doing both at once doubles the change blast radius and couples the substrate decision to retry semantics that are independent. Rejected; this ADR ships standalone, [FT-104](FT-104) consumes the new loop.

## Status

Proposed. Once accepted, governs the FT-123 implementation (in-process loop, retire `_subprocess_runner.py`). Amends [ADR-008](ADR-008) by adding `allowed_tools` to the dispatch payload. Aligns code-writer with [ADR-054](ADR-054) (worker SDK provider substrate) and [ADR-064](ADR-064) (LiteLLM proxy as the call substrate). Depends on [ADR-070](ADR-070) (role-scoped tool surfaces) for the source of truth, and on [ADR-071](ADR-071) (workspace containment + secrets blocking) for tool-call safety.
