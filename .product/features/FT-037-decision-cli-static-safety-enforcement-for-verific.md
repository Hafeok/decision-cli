---
id: FT-037
title: 'decision-cli: Static safety enforcement for verification graphs'
phase: 2
status: complete
depends-on:
- FT-035
- FT-036
adrs:
- ADR-028
tests:
- TC-058
- TC-059
domains: []
domains-acknowledged:
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-037 neither emits nor routes feedback.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-037 does not author or modify a fitness-function artifact.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-037 runs inside an already-scoped command and does not introduce a new scope check.
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-037 does not cross or alter that boundary.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-037 does not introduce or modify a role catalog entry.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-037 produces no feedback artifacts.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-037 runs after the working directory is resolved and does not re-discover it.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-037 produces no feedback artifacts.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-037 is out of scope for the pairing.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-037's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-037 produces no new Session or event type and inherits lineage from the harness.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-037's code is organised under that migration, not by this feature.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-037 neither emits nor consumes verdicts (verdict aggregation across safety violations is slice 3).
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-037 has no feedback to gate.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-037 produces no action/interpretation pair.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-037 reads/writes via the GraphWriter chokepoint and does not introduce event-sourced state.
---

## Description

The structural safety check that gates `VerificationGraph` persistence and dispatch per [ADR-028](ADR-028) §Safety gating: for every step in a graph, `step.requiredOps ⊆ env.allowedOps`. The check runs at authoring time (before `StreamWriter` persists the graph or a new step) and is exposed for pre-dispatch reuse in slice 3.

Substrate consumed by the graph and step CLI subcommand-features ([FT-041](FT-041), [FT-044](FT-044)).

## Functional Specification

### Inputs

- The step-kind vocabulary and `dec:requiredOps` declarations from [FT-036](FT-036).
- The environment `dec:allowedOps` and `dec:safetyClass` from [FT-035](FT-035).
- The `StreamWriter` chokepoint — extended with the check.

### Outputs

- A `core::verify::safety` module exposing:
  - `fn check_graph_against_env(graph: &VerificationGraph, env: &VerificationEnvironment) -> Result<(), SafetyViolation>`.
  - `fn check_step_against_env(step: &VerificationStep, env: &VerificationEnvironment) -> Result<(), SafetyViolation>` — single-step variant used by `step add`.
  - `struct SafetyViolation { step_id: StepIri, step_kind: StepKind, missing_ops: Vec<String>, env_id: EnvIri, env_allowed_ops: Vec<String>, env_safety_class: SafetyClass }`.
- A `core::handler::Error::SafetyViolation(SafetyViolation)` variant for surface-uniform error rendering.
- `StreamWriter` integration: when a `VerificationGraph` commit references an env, the writer fetches the env and runs the check before the SHACL pass. When a single `VerificationStep` is appended, the writer fetches the parent graph's env and runs the per-step variant.

### State

- None. The check is pure given the graph (or step) and the env it receives.

### Behaviour

1. Resolve the step's `requiredOps` from its `StepKind` using the declarations from [FT-036](FT-036). `sparql-assertion` is conditional: a local file `dec:target` requires `sparql-local`; an HTTP `dec:target` requires `sparql-http`.
2. Resolve the env's `allowedOps` from its `dec:allowedOps` list.
3. For each step (or the single step in the per-step variant), assert `requiredOps ⊆ allowedOps`. The first violation is returned with full diagnostic context; a `check_graph_against_env_all` variant returns every violation for batch authoring tools.
4. `StreamWriter` invokes the appropriate variant after Turtle parse and before SHACL validation. SHACL failure and safety failure are distinct error variants — the CLI / MCP renders them differently.
5. The same functions are exposed for slice-3 pre-dispatch reuse; same error type carries through.

### Invariants

- Safety enforcement is the only gate determining whether a graph may execute against an env. No other layer (executor, dispatch handler) re-implements the check.
- The check is deterministic: same `(graph, env)` always produces the same outcome.
- The check is op-direction-sensitive: `step.requiredOps ⊆ env.allowedOps`, never the reverse.
- The check runs whenever a graph or a step is persisted — never bypassed by `dec verify` authoring commands.

### Error handling

- Missing-op violation → `Error::SafetyViolation { step, missing_ops, env_allowed_ops, env_safety_class }`. CLI renders a human-friendly diff (e.g. "step `step:2` (kind `http-request`) requires `http-mutating`, but environment `env:prod-deployment` (safety class `production-readonly`) allows only `http-readonly`"). MCP returns the structured error.
- Env not found (graph references an env that doesn't exist) → delegated to [FT-036](FT-036)'s `DanglingRef` error; not surfaced as a safety violation.
- Unknown op token in either side → `Error::UnknownOp { token, source: "step" | "env" }`.

### Boundaries

- **In scope.** The structural check (graph + per-step variants), the error type, `StreamWriter` integration, the public API for slice-3 reuse.
- **Out of scope.** Runtime enforcement during step execution (slice 3 reuses the same function). Composition with autonomy levels (slice 3 dispatch handler reads both safety class and autonomy level). Op-vocabulary expansion (each new step kind adds its own `requiredOps` declaration via [FT-036](FT-036) or its successor).

## Out of scope

- Composition with autonomy levels (slice 3).
- Runtime op interception during step execution (slice 3).
- Per-stream safety overrides.
- Whitelist / denylist of specific op-target tuples (e.g. "allow http to dev.example.com but not example.com" — future feature).
