---
id: FT-048
title: 'decision-cli: verify-graph-author worker package'
phase: 2
status: planned
depends-on: []
adrs:
- ADR-030
tests:
- TC-076
- TC-077
- TC-078
domains: []
domains-acknowledged:
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-048 is a stateless Python worker with no graph or event access and does not cross that boundary.
  ADR-002: ADR-002 (graph-as-state) governs persistence semantics; FT-048 performs no persistence — it reads the bundle and writes only stdout.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; FT-048 is invoked under an existing dispatch session and produces no new lineage of its own.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; FT-048 receives a pre-scoped bundle and does not perform scope checks.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-048 is a worker subprocess invoked by an already-scoped CLI handler.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-048's Python code conforms to ruff format/check; ADR-013 itself is owned by FT-014.
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-048 does not author or modify a fitness-function artifact.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-048 lives under workers/ and is structurally separate from the Rust crate's SDP enforcement.
  ADR-017: ADR-017 (action-interpretation pairing) is a Slice 2 structural requirement implemented by FT-021; FT-048's GraphProposal is the action half of an action-interpretation pair completed by the slice-3 graph executor.
  ADR-018: ADR-018 (VerificationVerdict schema) is a Slice 2 artifact implemented by FT-020; FT-048 produces a GraphProposal, not a verdict.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; the slice-3 graph-executor pairing will measure agreement for this role, not this feature.
  ADR-022: ADR-022 (feedback as a first-class flow class) is a Slice 3 concern implemented by FT-026; FT-048's Gap output is structurally feedback-shaped but is routed via the worker output channel, not via the feedback flow.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-048 produces no feedback artifacts in slice 2.6.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-048 produces no feedback artifacts in slice 2.6.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-048 has no feedback to gate.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; the verify-graph-author role catalog entry is added under FT-030's pattern when it lands; FT-048 supplies the worker implementation, not the role-catalog entry.
---

## Description

The Python worker package implementing the `verify-graph-author` role defined in [ADR-030](ADR-030). Stateless single-shot Claude call: takes a `VerifyGraphAuthorInput` bundle (feature_spec + TCs + env catalog + existing-graph candidates + step vocabulary), returns a `GraphProposal` (`Match` / `New` / `Gap`). Mirrors the package shape of `workers/code-writer/` ([FT-013](FT-013)) and `workers/verifier/` ([FT-023](FT-023)).

The worker performs no graph access. Match-vs-generate is **not** a worker decision — the bundle assembler ([FT-049](FT-049)) runs [FT-046](FT-046) first and only invokes the worker when there is no complete match; the worker sees the matcher's candidate report as part of its bundle for rationale-shaping, but the decision to write a new graph is the harness's, not the LLM's.

One subcommand → one slice — for workers the "slice" is the package itself; the CLI surface lives in [FT-049](FT-049).

## Functional Specification

### Inputs

- A Pydantic `VerifyGraphAuthorInput` matching [ADR-030](ADR-030)'s bundle contract:
  ```python
  class VerifyGraphAuthorInput(BaseModel):
      feature_id: str
      feature_spec: str                          # full markdown body
      relevant_tcs: list[TcRecord]
      target_environment: EnvRecord              # exactly one env per call
      candidate_graphs: list[ExistingGraphRecord]
      step_vocabulary: list[StepKindRecord]
      bundle_hash: str
  ```
  - `TcRecord = { id, title, body }`.
  - `EnvRecord = { id, env_type, safety_class, allowed_ops, endpoint? }`.
  - `ExistingGraphRecord = { id, verifies, covers: list[str], step_summaries: list[StepSummary] }`.
  - `StepKindRecord = { kind, required_ops, fields_schema: dict, description }`.
- An invocation: `python -m verify_graph_author --bundle <path-to-input-json>` (entry point identical in shape to `code-writer`).
- Anthropic API key via env var (same convention as `code-writer`).

### Outputs

- A Pydantic `GraphProposal` printed to stdout as JSON:
  ```python
  class GraphProposal(BaseModel):
      kind: Literal["match", "new", "gap"]
      bundle_hash: str                           # echo for verification
      match: Optional[MatchProposal] = None
      new: Optional[NewProposal] = None
      gap: Optional[GapProposal] = None

  class MatchProposal(BaseModel):
      graph_id: str
      rationale: str

  class NewProposal(BaseModel):
      environment: str
      steps: list[ProposedStep]                  # ordered
      rationale: str

  class ProposedStep(BaseModel):
      step_type: Literal["shell-command", "sparql-assertion", "file-assertion",
                          "http-request", "wait-for", "capture"]
      fields: dict[str, Any]                     # per-kind, validated client-side after return
      provides_evidence_for: list[str]           # TC ids — drives coverage

  class GapProposal(BaseModel):
      uncovered_tcs: list[str]
      reason: str                                # why the step vocabulary is insufficient
  ```
- Exit 0 on a structured proposal returned (regardless of `kind`).
- Exit non-zero on infrastructure failure (network, malformed bundle, schema validation of `GraphProposal` failed) — these are not `Gap` cases, they are worker faults.

### State

- None. The worker is stateless; the bundle is the input, the proposal is the output. No graph access, no disk writes beyond stdout.

### Behaviour

1. Parse the input bundle from the supplied JSON path; validate against `VerifyGraphAuthorInput`.
2. Construct the prompt template with five sections:
   - **Goal.** "Propose a verification graph that produces evidence for each of the following TCs in the given environment."
   - **Feature.** Embedded `feature_spec` body and the list of TCs (id, title, body).
   - **Environment.** `target_environment` block — env type, safety class, allowed ops, endpoint if any.
   - **Vocabulary.** The step kinds available, each with `required_ops` and `fields_schema`. Explicit note: "You may only use the listed kinds. If your strategy needs operations not in `target_environment.allowed_ops`, return a `Gap`."
   - **Candidates.** The list of existing graphs that touch any of the feature's TCs in this env, each with their step summaries and which TCs they currently cover. Instruction: "If one of these graphs already adequately covers the feature's TCs, return a `Match` with its id and your rationale. Otherwise return a `New` graph. You may borrow patterns from candidates — name them in your rationale."
3. Call Claude (`anthropic.Anthropic().messages.create(...)`) with structured-output (tool-use) constraint matching the `GraphProposal` schema.
4. Validate the response shape against `GraphProposal`. On schema failure (worker fault, not LLM judgement) → retry once with the schema-violation diagnostic appended to the prompt, then fail.
5. Echo `bundle_hash` in the output so the harness can verify the proposal pairs to the bundle it sent.
6. For `New` proposals: validate each `ProposedStep`'s `fields` against the kind's `fields_schema` *inside the worker*; if a step fails its kind schema, retry once with a diagnostic, then return a `Gap` rather than producing an invalid `New` (the worker prefers honest failure to garbage output).
7. Print the validated `GraphProposal` JSON to stdout; exit 0.

### Invariants

- Stateless — no module-level state, no filesystem writes except stdout.
- No graph access — the bundle is the only source of truth about the feature, its TCs, the env, and existing graphs. The worker does not touch `.dec/` directly.
- The output schema is **strict** — any deviation from `GraphProposal` is a worker fault, not a low-confidence judgement.
- `Gap` is a first-class outcome. The worker prefers to return `Gap` over `New` whenever the proposed steps would not cover all `relevant_tcs` or would require ops outside `allowed_ops`.
- Single-shot — one Claude call per invocation (plus at most one retry on schema-validation failure). The worker does not loop, does not iterate, does not "improve" proposals.
- `provides_evidence_for` is **required** on every step in a `New` proposal; an empty list is an explicit "this step is a setup or capture, evidence is elsewhere" rather than a missing field. The worker must justify any step with empty `provides_evidence_for` in the `rationale`.

### Error handling

- Bundle missing required field → exit 2 with structured error.
- Bundle's `bundle_hash` malformed → exit 2.
- Claude API failure (network, auth, rate limit) → exit 3 with structured error; the harness will surface and may retry the *invocation*, not the LLM call.
- Schema validation failure on Claude response after one retry → exit 4 with structured error including the failing response (for debug).
- `GraphProposal.bundle_hash` ≠ input `bundle_hash` (impossible barring a worker bug) → exit 5.

### Boundaries

- **In scope.** Package layout (`workers/verify-graph-author/`), `pyproject.toml`, the Pydantic models, the prompt template, the Anthropic call, structured-output validation, the `__main__` entry, ruff + pytest scaffolding, a unit test that exercises a mocked-Claude path against a synthetic bundle.
- **Out of scope.** Persisting the proposed graph (that is the bundle assembler / CLI in [FT-049](FT-049)). Running the matcher ([FT-046](FT-046) runs *before* the worker, not inside it). Auto-acceptance / Level-4 graduation ([ADR-030](ADR-030) §7; out of slice 2.6 scope). Multi-environment composite proposals (one env per call, per [ADR-030](ADR-030)).

## Out of scope

- Persistence — the worker prints JSON, never writes graph artifacts.
- Multi-environment proposals.
- Auto-acceptance / auto-write of proposals.
- Calling Claude more than twice per invocation (retry budget is 1).
- Graph access from inside the worker.
- Worker-side coverage computation against the live store ([FT-045](FT-045) is the authority; the worker uses its `coverage_report` field from the bundle only).
