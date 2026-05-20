---
id: FT-023
title: Python verifier worker
phase: 2
status: complete
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
domains-acknowledged:
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-023's code is reorganised under that migration, not by this feature.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-023's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-023 neither emits nor consumes verdicts.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-023 produces no feedback artifacts.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-023 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-023 neither emits nor routes feedback.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-023 produces no new Session or event type and inherits lineage from the harness.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-023 has no feedback to gate.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-023 produces no action/interpretation pair.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-023 is out of scope for the pairing.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-023 produces no feedback artifacts.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-023 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-023 does not cross or alter that boundary.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-023 does not introduce or modify a role catalog entry.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-023 runs after the working directory is resolved and does not re-discover it.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-023 does not author or modify a fitness-function artifact.
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
