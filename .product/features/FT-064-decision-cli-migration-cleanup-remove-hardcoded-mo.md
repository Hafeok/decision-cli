---
id: FT-064
title: 'decision-cli: Migration cleanup — remove hardcoded model bindings from worker layer'
phase: 2
status: complete
depends-on:
- FT-060
- FT-061
- FT-062
- FT-063
- FT-065
adrs:
- ADR-008
- ADR-015
- ADR-017
- ADR-018
- ADR-020
- ADR-021
- ADR-022
- ADR-023
- ADR-024
- ADR-025
- ADR-027
- ADR-033
- ADR-034
- ADR-035
- ADR-036
- ADR-037
tests:
- TC-113
domains:
- api
- security
domains-acknowledged: {}
---

## Description

The final step in the PRD's implementation order: remove every hardcoded model binding from the worker layer so all role-to-model resolution flows through the catalog. The verifier worker today carries `DEFAULT_MODEL_ID = "claude-sonnet-4-5"` at `workers/verifier/src/verifier/worker.py:17` and a `VERIFIER_MODEL_ID` env-var escape hatch. The code-writer worker today delegates to `claude -p` headless subprocess where the model is implicit in the Claude Code installation. Both paths must be replaced with capability-resolved dispatch payloads.

The verifier migration is straightforward — the `ModelCaller` seam is already present and [FT-060](FT-060) lands the `ModelRouter` abstraction. The code-writer migration is the harder lift: it requires either (a) keeping `claude -p` for `endpoint=anthropic` bindings and adding an SDK-driven agentic harness for `endpoint=scaleway` bindings, or (b) replacing `claude -p` entirely with the SDK-driven harness even for Anthropic. This feature ships option (a): a dual path with `claude -p` retained for the Anthropic case and a new SDK-driven harness (using [FT-060](FT-060)'s `ModelRouter`) for Scaleway.

## Functional Specification

### Inputs

- The verifier worker at `workers/verifier/src/verifier/worker.py` (uses anthropic SDK directly).
- The code-writer worker at `workers/code-writer/src/code_writer/` (uses `claude -p` subprocess).
- The `ModelRouter` from [FT-060](FT-060).
- The dispatch payload schema from [FT-061](FT-061) carrying `endpoint`, `model_identifier`, `parameters`, `capability_ref`.
- The role catalog from [FT-058](FT-058) with seeded bindings for `implementer` (Scaleway) and `verifier` (Scaleway).

### Outputs

**Verifier worker changes:**

- Remove `DEFAULT_MODEL_ID = "claude-sonnet-4-5"` from `worker.py:17`.
- Remove `MODEL_ENV_VAR = "VERIFIER_MODEL_ID"` env-var resolution (env override no longer needed; bindings are graph-resident per [ADR-033](ADR-033)).
- Replace `resolve_model_id(bundle)` with reading `bundle.endpoint`, `bundle.model_identifier`, `bundle.parameters` from the dispatch payload.
- Replace `call_claude` with `ModelRouter.call` from [FT-060](FT-060).
- `STUB_ENV_VAR = "VERIFIER_STUB"` remains — stub mode is testing infrastructure, not policy.

**Code-writer worker changes:**

- The existing `claude -p` subprocess path moves into a new `AnthropicAgenticRunner` ([FT-060](FT-060)-adjacent) and remains the runner for `endpoint=anthropic` bindings (using `--model <model_identifier>` from the dispatch payload).
- A new `ScalewayAgenticRunner` uses [FT-060](FT-060)'s `ModelRouter` to drive a tool-using loop:
  1. Build a system prompt from the bundle.
  2. Build the canonical tool list ([FT-060](FT-060)'s `tools.py`: read_file, edit_file, run_bash, record_emergent_judgment, file_feedback, submit).
  3. Call the model with tools. For each tool-use block in the response, execute the tool against the workspace, append the result to the conversation, and call again until the model emits `submit` or hits a turn cap (configured per dispatch, default 50).
  4. Construct a `CodeChange` from the observed `edit_file` / `write_file` calls and the `submit` payload.
- The runner is selected by dispatch payload's `endpoint`:
  ```python
  def select_runner(payload) -> AgenticRunner:
      match payload.endpoint:
          case "anthropic": return AnthropicAgenticRunner()
          case "scaleway":  return ScalewayAgenticRunner()
          case other:       raise WorkerError(f"unknown endpoint: {other}")
  ```
- The `--worker-command` / `$CODE_WRITER_CMD` env override from FT-016 / ADR-015 remains intact (worker *binary* selection is a separate axis from model selection).

**Migration verification:**

- After the migration, `grep -r "claude-sonnet" workers/` returns no matches (search ignores tests that explicitly test the migration removed it).
- `grep -r "claude-opus" workers/` returns no matches.
- `grep -r "anthropic.Anthropic()" workers/` returns matches only inside [FT-060](FT-060)'s `model_router.py` (the central Anthropic client construction) and inside test fixtures.

### State

- No persistent state changes; existing in-flight sessions complete normally.
- The verifier's stub mode is retained for CI / fixture runs.

### Behaviour

1. The verifier worker's entry point reads the new dispatch payload format, constructs a `ModelRouter` for the endpoint, and dispatches the verification per [FT-060](FT-060)'s shape.
2. The code-writer worker's entry point selects an `AgenticRunner` per the dispatch payload's endpoint and dispatches.
3. The Anthropic agentic runner is the existing `claude -p` code path, refactored to read `--model` from the dispatch payload rather than relying on Claude Code's default. No model identifier means the runner falls back to whatever the operator's Claude Code installation defaults to — but this path should never trigger if [FT-058](FT-058) seeded bindings correctly.
4. The Scaleway agentic runner is new code; the bulk of the work in this feature is implementing the tool-call loop against the `ModelRouter`. Tool implementations (read_file, edit_file, run_bash, etc.) are shared between runners — they are workspace operations, not endpoint operations.
5. PRD §11.5 invariants are checked: existing in-flight sessions complete; rolling back means reverting the migration commit; catalog/binding artifacts persist as harmless orphans.

### Invariants

- After this feature lands, no worker reads `ANTHROPIC_API_KEY` or `SCW_SECRET_KEY` outside of [FT-059](FT-059)'s wrapper and [FT-060](FT-060)'s router.
- After this feature lands, the only places model identifiers appear as string constants in the worker layer are: (1) `workers/_shared/src/_shared/model_router.py` (where each router knows how to call its endpoint, but no model id is hardcoded there), (2) test fixtures that explicitly pin a model for a specific test.
- The worker contract is unchanged: bundle in, artifact out. Workers still don't talk to the graph ([ADR-008](ADR-008)).
- Existing TCs for [FT-013](FT-013), [FT-023](FT-023), [FT-048](FT-048) still pass — the migration changes the *plumbing*, not the *behaviour* (a Claude-routed dispatch still produces the same `CodeChange` / `VerificationVerdict` it produced before).

### Error handling

- Endpoint string in dispatch payload that is not in the known set → `WorkerError("unknown endpoint: {x}")`; session recorded as failed.
- The Scaleway runner hits the turn cap without a `submit` → session recorded as `turn_cap_exhausted`; `CodeChange` constructed from whatever was edited; the dispatcher's escalation policy ([FT-062](FT-062)) may then escalate.
- Anthropic runner subprocess error: existing path (per [FT-013](FT-013)) — captured and reported as session telemetry.
- Tool-call execution failure (e.g. `edit_file` outside workspace): runner aborts the dispatch with a structured error; signals to the dispatcher include `audit_fail = true`.

### Boundaries

- **In scope.** Verifier worker refactor, code-writer dual-runner architecture, Scaleway agentic loop, migration verification.
- **Out of scope.** [FT-059](FT-059) client wrapper, [FT-060](FT-060) router abstraction, [FT-061](FT-061)/[FT-062](FT-062) dispatcher changes — they are prerequisites.
- **Out of scope.** Verify-graph-author worker ([FT-048](FT-048)) migration — same pattern applies; can follow this feature or be bundled as a small follow-up.
- **Out of scope.** Performance optimisation of the new Scaleway tool-call loop (Phase 3 work).

## Out of scope

- Removing the `--worker-command` env-var override from FT-016 / ADR-015. That mechanism is about *which binary* runs the role, not *which model* the binary calls; it stays.
- Replacing `claude -p` with the SDK-driven agentic runner even for Anthropic bindings. The `claude -p` path works and uses subscription auth (no API-key spend); keeping it as the Anthropic runner preserves an existing cost optimisation. A future feature_spec can revisit if the dual runner becomes a maintenance burden.
- New worker types beyond the existing harness (PRD §3 out-of-scope).
- Replacing Anthropic entirely (PRD §3 — Opus 4.7 stays as the tier-3 deep-reasoning capability).
