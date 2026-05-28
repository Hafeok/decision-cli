---
id: FT-011
title: 'decision-cli: Implementer role end-to-end'
phase: 1
status: complete
depends-on:
- FT-003
- FT-004
- FT-009
- FT-010
- FT-013
adrs:
- ADR-004
- ADR-005
- ADR-008
- ADR-009
- ADR-010
- ADR-001
- ADR-002
- ADR-011
- ADR-012
tests:
- TC-008
- TC-010
- TC-012
- TC-013
domains: []
domains-acknowledged: {}
---

## Description

The implementer role is wired end-to-end in slice 1. Triggered explicitly per **ADR-010**, the harness assembles a bundle via subprocess to product-cli per **ADR-009**, dispatches it to the Python code-writer worker (FT-013) following the stateless contract in **ADR-008**, records a `Session` with PROV-O lineage per **ADR-004** linked to the active ValueStream via `dec:inStream` per **ADR-005**, and registers the resulting `CodeChange` in product-cli's graph via MCP.

Slice 1 uses hardcoded model binding (one Claude model) and a hardcoded policy. Model catalog and policy artifacts are deferred per `decision-cli-slice-1-bounds.md` §6.2.

See `decision-cli-slice-1-bounds.md` §6.1, §7, §8.

## Functional Specification

### Inputs

- A feature id from `dec implement FT-XXX` (FT-012).
- The active ValueStream (FT-010).
- A configured product-cli installation reachable as subprocess (ADR-009).
- A configured worker (FT-013) reachable via the dispatch event substrate (FT-003 + FT-004).

### Outputs

- A persisted `Session` artifact with PROV-O lineage (ADR-004).
- A persisted `Dispatch` artifact linking Session to worker run.
- A `CodeChange` artifact registered in product-cli's graph via MCP (ADR-009).
- Files written by the worker into the workspace.
- "Dispatch available" and "dispatch completed" events emitted (FT-003/FT-004).

### State

- Per-dispatch in-memory tracker: dispatch id, bundle hash, start time, worker handle.
- The orchestration store persists Session and Dispatch artifacts.

### Behaviour

1. Validate the feature id; validate the active stream's authorized-goals (ADR-005 via FT-010).
2. Invoke `product context FT-XXX --depth N` as subprocess (ADR-009); compute the bundle's content hash.
3. Mint a `Session` artifact with `dec:inStream` (ADR-005), bundle hash, model id, timestamps; PROV-O links per ADR-004.
4. Mint a `Dispatch`; emit "dispatch available for code-writer" event.
5. Wait for the worker's structured response (configurable timeout) — worker conforms to ADR-008.
6. On success: write `CodeChange` via product-cli's MCP write tool (ADR-009); update Session with token counts, duration, output ref; mark Dispatch complete; emit "dispatch completed".
7. On worker error: record on Session, include the worker's `error.detail` so the failure reason is visible to the operator (not just the category); mark Dispatch failed; emit completion event with failure.

### Dispatch payload shape

The harness passes the worker a JSON payload containing:

- `dispatch_id`, `session_id`, `feature_id`, `bundle_markdown`, `bundle_hash`, `workspace_path`, `model_id`.
- `timeout_seconds` — the only upper bound on the headless agent's runtime in slice 1.
- `max_turns` and `allowed_tools` are **omitted in slice 1**. The worker invokes `claude -p --dangerously-skip-permissions` with no turn cap so the headless agent runs to completion, matching `product-cli`'s working `product implement --headless` invocation. Re-introducing turn caps and tool allowlists is deferred until policy artifacts (ADR-010) land.

### Invariants

- Every `Session` links to bundle hash, model version, and ValueStream via PROV-O (ADR-004, ADR-005).
- Every `CodeChange` has a corresponding `Session` reachable via PROV-O (ADR-004).
- Failed dispatches still produce a Session record — no silent failures.
- A worker failure's `error.detail` is preserved on the Session and surfaced in the CLI failure message.

### Error handling

- product-cli subprocess failure → Session with `failed-at-bundle-assembly`; no dispatch.
- Worker timeout → Dispatch failed; completion event emitted; error on Session.
- MCP write failure → Session records the rejection reason; CodeChange not persisted in product-cli.

### Boundaries

- Does NOT define worker internals (FT-013, ADR-008).
- Does NOT define the CLI surface (FT-012, ADR-011).
- Does NOT mutate product-cli's graph directly — only via MCP (ADR-009).

## Out of scope

- Multi-role flow.
- Structured feedback emission from worker (ADR-008 defers to slice 2).
- Model catalog and policy as graph artifacts.
- Interpretation pairing (verification session paired with action).
