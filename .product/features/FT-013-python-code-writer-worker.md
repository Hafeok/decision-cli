---
id: FT-013
title: Python code-writer worker
phase: 1
status: planned
depends-on:
- FT-004
adrs:
- ADR-008
- ADR-001
- ADR-002
- ADR-004
- ADR-005
- ADR-012
tests: []
domains: []
domains-acknowledged: {}
---

## Description

The Python code-writer worker conforms to **ADR-008 (Worker contract: stateless bundle-in, artifact-out)**: receives a serialised bundle via a dispatch event (FT-003 / FT-004), calls Claude with structured output, writes files directly to the workspace, returns a `CodeChange` artifact.

Workers do not talk to the graph (ADR-008). Slice 1 workers report errors as session telemetry but do not emit structured feedback (deferred per ADR-008).

See `decision-cli-slice-1-bounds.md` §7.

## Functional Specification

### Inputs

- A dispatch event payload over SSE (FT-004): dispatch id, bundle markdown, target workspace path, model id, run parameters (max tokens, temperature, etc.).
- Anthropic API credentials via environment variable.

### Outputs

- A structured `CodeChange` (Pydantic model): file paths written, per-file diff summary, optional summary message.
- Session telemetry: tokens, latency, tool-call history, errors.
- Files written directly to the configured workspace directory.

### State

- Per-dispatch ephemeral state only — token counters, retry counters. Nothing persisted between dispatches (ADR-008).

### Behaviour

1. Subscribe to SSE; filter for "dispatch available for code-writer" events targeting this worker.
2. On receipt, parse the bundle payload.
3. Call Claude (anthropic SDK) with structured output configured to the `CodeChange` schema.
4. Write each file in the response to the workspace.
5. Publish a "dispatch completed" payload via the harness response channel (harness handles graph mutation per FT-011).
6. Report telemetry alongside completion.

### Invariants

- The worker never reads from or writes to the Oxigraph store directly (ADR-008).
- A worker run produces exactly one `CodeChange` (success) or one structured error (failure).
- Files written are confined to the configured workspace path; path traversal is rejected.

### Error handling

- Anthropic API error (rate limit, 5xx, malformed structured output) → structured error with category and retry hint; harness decides retry.
- File write failure → abort dispatch, report error; no partial state reported as success.
- Timeout (configurable) → terminate Claude call cleanly; report timeout.

### Boundaries

- Worker does NOT decide what feature to work on (harness via `dec implement`).
- Worker does NOT mutate the graph (ADR-008).
- Worker does NOT emit structured feedback in slice 1 (ADR-008).

## Out of scope

- Multi-turn conversational refinement within a single dispatch.
- Tool use beyond Anthropic SDK structured output.
- Git operations — slice 1 writes files directly.
- Structured feedback emission (ADR-008 defers to slice 2).
