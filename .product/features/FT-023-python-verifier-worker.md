---
id: FT-023
title: Python verifier worker
phase: 2
status: planned
depends-on:
- FT-013
- FT-020
- FT-022
adrs:
- ADR-008
- ADR-020
tests:
- TC-029
- TC-030
domains: []
domains-acknowledged: {}
---

## Description

The Python verifier worker — sibling of `workers/code-writer/`, same worker contract per [ADR-008](ADR-008) and [ADR-020](ADR-020): stateless, bundle-in / artifact-out, single-shot Claude call. Receives a verifier bundle and produces a `VerificationVerdict` ([ADR-018](ADR-018)) consumed by the orchestrator via [FT-022](FT-022)'s delivery transport.

## Functional Specification

### Inputs

- A serialised verifier bundle delivered via the dispatch event from [FT-022](FT-022). Bundle contains: produced artifact (the `CodeChange` text), originating feature_spec markdown, bundle hash that produced the action, relevant TCs (id + type + body), relevant cross-cutting ADRs (id + scope + body), dispatch group IRI for PROV-O.
- The verifier system prompt (lives in `workers/verifier/code/prompts.py`).
- The Claude API (via the `anthropic` SDK; same model binding hardcoded per [ADR-020](ADR-020)).

### Outputs

- A `VerificationVerdict` Pydantic model serialised as JSON on stdout (matches `code-writer`'s stdout protocol from [FT-013](FT-013)).
- Session telemetry: tokens, latency, model identifier, exit reason.

### State

- Stateless. No file writes. No graph access ([ADR-008](ADR-008) — invariant).

### Behaviour

1. Package shape:
   ```
   workers/verifier/
     pyproject.toml
     verifier/
       __init__.py
       __main__.py        # CLI entry point: read bundle from stdin, write verdict to stdout
       bundle.py          # parse VerifierInput (Pydantic)
       worker.py          # call Claude with structured output, return VerificationVerdict
       output.py          # serialise VerificationVerdict to the harness's expected JSON shape
       prompts.py         # system prompt for the verifier role
   ```
2. Parse `VerifierInput` from stdin (matches the format the harness produces in [FT-022](FT-022)).
3. Call Claude once with the verifier system prompt and the bundle content. Use structured output (Pydantic schema for `VerificationVerdict`).
4. Validate the model's response against the Pydantic schema. Refuse unknown fields, missing required fields, out-of-vocabulary verdicts.
5. Serialise the validated `VerificationVerdict` JSON to stdout.
6. Exit 0 on success, 1 on parse failure / Claude error / validation failure with diagnostic to stderr.
7. Conform to the worker contract invariants from [ADR-008](ADR-008):
   - No imports from `decision_cli` or any project Rust crate.
   - No file writes (the verifier does NOT touch the workspace).
   - No graph access.
8. Install as a `uv tool install` package, matching `code-writer`'s installation pattern (per the auto-memory note about `code-writer` being a uv tool install).

### Invariants

- Worker exits 0 iff it produced a Pydantic-validated `VerificationVerdict` on stdout.
- Worker writes nothing to the workspace.
- Worker imports no project Rust modules.
- Worker's output JSON includes all required fields per [ADR-018](ADR-018) (verdict, rationale ≥ 20 chars, violates if rejected/amendment-required, amendmentGuidance if amendment-required).

### Error handling

- Stdin parse error → exit 1; stderr: `verifier: bundle parse error: <detail>`.
- Claude API error → exit 1; stderr: `verifier: model call failed: <detail>`.
- Pydantic validation error on Claude's response → the worker re-prompts ONCE with the schema violation message; if the second response also fails, exit 1.
- Empty rationale or ≤ 20 char rationale → re-prompt once (the model often inflates on demand); if still short, exit 1 with `verifier: rationale below minimum length`.

### Boundaries

- **In scope.** The verifier worker package: bundle parser, Claude call, output serialiser, system prompt, packaging.
- **Out of scope.** Bundle assembly ([FT-022](FT-022)'s delivery handler). Harness-side write of the verdict ([FT-021](FT-021)'s orchestrator). SHACL validation ([FT-020](FT-020)'s `StreamWriter`). CLI surface ([FT-025](FT-025)).

## Out of scope

- Tool use beyond a single LLM call (rejected per ADR-020).
- Multi-turn verifier conversations (rejected per ADR-020).
- A Rust verifier binary (rejected per ADR-020).
- Asymmetric model selection (e.g. Sonnet write / Opus verify) — Phase B at earliest.
