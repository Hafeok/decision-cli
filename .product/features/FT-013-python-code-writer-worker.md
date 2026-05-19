---
id: FT-013
title: Python code-writer worker
phase: 1
status: complete
depends-on:
- FT-004
adrs:
- ADR-008
- ADR-001
- ADR-002
- ADR-004
- ADR-005
- ADR-012
tests:
- TC-008
- TC-011
- TC-013
domains: []
domains-acknowledged:
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-013's code is reorganised under that migration, not by this feature.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-013 produces no action/interpretation pair.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-013 does not author or modify a fitness-function artifact.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-013's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-013 produces no feedback artifacts.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-013 does not introduce or modify a role catalog entry.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-013 neither emits nor routes feedback.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-013 has no feedback to gate.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-013 is out of scope for the pairing.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-013 produces no feedback artifacts.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-013 neither emits nor consumes verdicts.
---

## Description

The Python code-writer worker conforms to **ADR-008 (Worker contract: stateless bundle-in, artifact-out)**: it receives a serialised bundle via a dispatch event (FT-003 / FT-004), invokes Claude as a headless subprocess (`claude -p`), observes the resulting file writes and final summary, and returns a `CodeChange` artifact.

The worker delegates the model call to the **Claude Code CLI in headless print mode (`claude -p`)** rather than the Anthropic API SDK. This re-uses the operator's existing Claude subscription authentication — no `ANTHROPIC_API_KEY` is required, no per-token billing accrues against an API key, and Claude Code's built-in editing tools are available inside the session.

Workers do not talk to the graph (ADR-008). Slice 1 workers report errors as session telemetry but do not emit structured feedback (deferred per ADR-008).

See `decision-cli-slice-1-bounds.md` §7.

## Functional Specification

### Inputs

- A dispatch event payload over SSE (FT-004): dispatch id, bundle markdown, target workspace path, model id, optional run parameters (timeout; max_turns/allowed_tools deferred — see below).
- A working Claude Code installation on `$PATH` (`claude` binary) with an authenticated subscription session on the host.
- No Anthropic API key is required or read.

### Outputs

- A structured `CodeChange` (Pydantic model): file paths written, per-file diff summary, optional summary message.
- Session telemetry: turn count, latency, tool-call history (parsed from `claude -p --output-format stream-json --verbose`), errors.
- Files written directly to the configured workspace directory — written by Claude Code's tool calls inside the headless session and observed by the worker.

### State

- Per-dispatch ephemeral state only — turn counters, retry counters, the subprocess handle, and a temp file holding the bundle (system-prompt-file). Nothing persisted between dispatches (ADR-008).
- Each dispatch spawns a fresh `claude -p` subprocess scoped to the configured workspace; no Claude Code session state is reused across dispatches in slice 1.

### Behaviour

1. Subscribe to SSE; filter for "dispatch available for code-writer" events targeting this worker.
2. On receipt, parse the bundle payload and write the bundle markdown to a per-dispatch temp file.
3. Spawn `claude -p` as a subprocess with:
   - `--system-prompt-file <tempfile>` so the bundle is the model's system prompt (matches product-cli's `product implement --headless` invocation),
   - a short user message instructing the model to implement the feature and run verification when done,
   - `--dangerously-skip-permissions` so tool-call permission gates do not stall the headless session,
   - `--output-format stream-json --verbose` so tool-call and result events can be parsed deterministically,
   - working directory set to the configured workspace path so file edits land in the right place,
   - `--max-turns` is **not** set — the agent runs to completion (matches product-cli's headless pattern). The dispatch payload's `timeout_seconds` provides the upper bound.
4. Stream stdout, parse the structured events into per-turn telemetry and a final result block.
5. Build a `CodeChange` from the observed file writes (Edit / Write tool calls) and the model's final summary.
6. Publish a "dispatch completed" payload via the harness response channel (harness handles graph mutation per FT-011).
7. Report telemetry alongside completion.

### Invariants

- The worker never reads from or writes to the Oxigraph store directly (ADR-008).
- The worker never calls the Anthropic API directly; all model traffic goes through `claude -p` (subscription auth).
- A worker run produces exactly one `CodeChange` (success) or one structured error (failure).
- Files written are confined to the configured workspace path. Claude Code is invoked with its working directory set to that path; the worker additionally validates that every recorded write path is inside the workspace and rejects traversal.

### Error handling

- `claude` binary missing on `$PATH`, or unauthenticated session → structured error (category `subscription_unavailable`); harness surfaces to operator.
- Subprocess exit non-zero → structured error (category `subprocess_failed`). `claude -p` emits its terminal error as a JSON `result` event on stdout (not stderr); when stderr is empty the worker scrapes the final `result` event from stdout and includes `subtype`, `terminal_reason`, `num_turns`, and `errors[]` in `error.detail` so failures are diagnosable instead of mystery exit codes.
- Unparseable stream-json → structured error with stdout/stderr capture; harness decides retry.
- File write rejected (outside workspace) → abort dispatch, report error; no partial state reported as success.
- Timeout (configurable via dispatch payload) → terminate the `claude -p` subprocess cleanly (SIGTERM, then SIGKILL); report timeout.

### Boundaries

- Worker does NOT decide what feature to work on (harness via `dec implement`).
- Worker does NOT mutate the graph (ADR-008).
- Worker does NOT emit structured feedback in slice 1 (ADR-008).
- Worker does NOT manage Claude Code authentication — that is operator setup (run `claude login` once on the host).

## Out of scope

- Direct Anthropic API SDK usage (explicitly replaced by `claude -p` headless sessions).
- Multi-turn interactive refinement across dispatches (each dispatch is a fresh headless session).
- Persisted Claude Code session resumption (`--resume`) — deferred.
- Git operations — slice 1 writes files directly via Claude Code's tools.
- Structured feedback emission (ADR-008 defers to slice 2).
- Capping turns or restricting tool allowlists in slice 1 — the headless agent runs unbounded under `--dangerously-skip-permissions` to match product-cli's working `--headless` flow. Re-introducing turn/tool caps is deferred to a later slice once policy artifacts (ADR-010) land.
