---
id: FT-126
title: 'decision-cli: tc-author worker package — authors TC bodies + runner fields for under-covered features'
phase: 4
status: planned
depends-on:
- FT-048
- FT-067
- FT-021
- FT-119
adrs:
- ADR-073
- ADR-074
- ADR-072
- ADR-070
tests:
- TC-286
- TC-287
- TC-288
- TC-289
domains: []
domains-acknowledged:
  ADR-071: ADR-071 governs in-process worker tool calls (workspace containment + secrets blocking for tools invoked inside an in-process agentic loop per FT-123). FT-126 is an out-of-process Python subprocess that makes one Anthropic API call via the SDK and writes only to stdout — no in-process tool surface, no filesystem access, no shell-out. The workspace-containment concern does not apply to this worker package.
---

## Description

Python worker package implementing the `tc-author` role established by
[ADR-073](ADR-073): the action half of the TC authoring pair. Takes a
feature_id + feature_spec + existing TCs + the TC schema, returns a
`TcProposal` (`sufficient` / `augment` / `new`). The harness writes the
proposed TCs via `product test new` + `product test runner` only after the
paired `tc-quality` judge ([FT-127](FT-127)) returns an `approved`
[QualityVerdict](ADR-074) and acceptance autonomy ([ADR-075](ADR-075)) flips
the readiness bit.

Mirrors the package shape of [FT-048](FT-048) (verify-graph-author) and
[FT-013](FT-013) (code-writer) exactly: stateless, single-shot, bundle-in /
artifact-out, no graph access, Pydantic strict I/O, structured-output Claude
call, retry budget 1, `python -m tc_author --stdin` entry point. The
match-vs-generate decision lives in the harness, not the worker; the
matcher is the existing [ADR-072](ADR-072) floor check ("does this feature
already meet `min_tcs_per_feature`?"). When the matcher's answer is "yes,"
the worker is invoked only with `sufficient` as the acceptable outcome.
When the answer is "no," the worker produces an `augment` (add to partial
coverage) or `new` (write from scratch) proposal.

One subcommand → one slice. The CLI/MCP surface that dispatches this worker
lives in the planner extension ([FT-131](FT-131)); this feature is the
worker package itself.

## Functional Specification

### Inputs

- A Pydantic `TcAuthorInput` carrying the bundle assembled by the harness:
  ```python
  class TcAuthorInput(BaseModel):
      feature_id: str
      feature_spec: str                          # full markdown body
      existing_tcs: list[TcRecord]
      tc_schema: TcSchemaRecord                  # field cardinalities, allowed runners
      runner_vocabulary: list[RunnerKindRecord]  # bash | cargo-test | pytest | custom
      target_count: int                          # min_tcs_per_feature from ADR-068/072
      coverage_axes: list[CoverageAxisRecord]    # advisory: happy / edge / integration / state
      bundle_hash: str
  ```
  - `TcRecord = { id, title, status, runner, runner_args, body }`.
  - `TcSchemaRecord` mirrors the TC frontmatter contract enforced by
    product-cli (per [ADR-047](ADR-047) feature-body completeness pattern,
    applied to TCs).
  - `RunnerKindRecord = { kind, args_pattern, timeout_default }` — the
    controlled vocabulary of runner kinds from
    [ADR-013](ADR-013) two-tier exit-code contract.
- An invocation: `python -m tc_author --stdin` (entry point identical in
  shape to FT-048's `verify-graph-author`).
- Anthropic API key via env var (same convention as `code-writer` and
  `verify-graph-author`).

### Outputs

- A Pydantic `TcProposal` printed to stdout as JSON:
  ```python
  class TcProposal(BaseModel):
      kind: Literal["sufficient", "augment", "new"]
      bundle_hash: str                           # echo for verification
      sufficient: Optional[SufficientProposal] = None
      augment: Optional[AugmentProposal] = None
      new: Optional[NewProposal] = None

  class SufficientProposal(BaseModel):
      reasoning: str                             # why existing TCs already meet target_count
      coverage_map: dict[str, list[str]]         # TC-id -> coverage_axes hit

  class AugmentProposal(BaseModel):
      retained_tcs: list[str]                    # TC-ids kept as-is
      additions: list[ProposedTc]                # new TCs to add
      reasoning: str

  class NewProposal(BaseModel):
      replaced_tcs: list[str]                    # TC-ids the harness should supersede
      tcs: list[ProposedTc]                      # ordered, target_count entries
      reasoning: str

  class ProposedTc(BaseModel):
      title: str
      type: Literal["scenario", "exit-criteria", "invariant"]
      body: str                                  # full markdown body, H2 sections per TC schema
      runner: Literal["bash", "cargo-test", "pytest", "custom"]
      runner_args: str
      runner_timeout: str                        # e.g. "60s"
      observes: list[str]                        # FT-072 observed surfaces
      coverage_axis: Optional[str]               # advisory tag from coverage_axes list
      validates_features: list[str]              # always includes feature_id
      validates_adrs: list[str]                  # cross-cutting ADRs cited in the spec
  ```
- Exit 0 on a structured proposal returned (regardless of `kind`).
- Exit non-zero on infrastructure failure (network, malformed bundle,
  schema validation of `TcProposal` failed).

### State

- None. Stateless; the bundle is the only input. No graph access, no disk
  writes beyond stdout. The harness — not this worker — performs all
  product-cli writes after the paired judge ([FT-127](FT-127)) returns an
  approved [QualityVerdict](ADR-074).

### Behaviour

1. Parse the input bundle from stdin (or the supplied JSON path); validate
   against `TcAuthorInput`.
2. Construct the prompt template with five sections, mirroring
   [FT-048](FT-048)'s template shape:
   - **Goal.** "Propose at least `target_count` distinct TCs that
     collectively cover the feature_spec across the documented coverage
     axes. Each TC must carry a wired runner; a TC frontmatter and its
     test must agree per CLAUDE.md."
   - **Feature.** Embedded `feature_spec` body.
   - **Existing TCs.** `existing_tcs` with their bodies and runners.
   - **Schema.** The TC frontmatter contract (allowed types, required
     observes, runner kinds and their `args_pattern`).
   - **Coverage axes (advisory).** The four axes from
     [ADR-072](ADR-072) (happy, edge, integration, state). The worker
     is asked to spread `target_count` TCs across distinct axes where the
     feature naturally admits multi-axis coverage; if it cannot, return
     a `Gap`-style failure in the rationale rather than padding.
3. Call Claude with structured-output constraint matching the `TcProposal`
   schema. Retry budget 1 on schema-validation failure.
4. Validate the response shape against `TcProposal`. For `augment` and
   `new`, validate each `ProposedTc.runner_args` against the
   `runner_vocabulary[runner].args_pattern`; on mismatch retry once with
   a diagnostic, then fall back to a `sufficient` kind with reasoning
   "could not produce wireable TCs."
5. Echo `bundle_hash` in the output so the harness can verify the proposal
   pairs to the bundle it sent.
6. Print the validated `TcProposal` JSON to stdout; exit 0.

### Invariants

- Stateless — no module-level state, no filesystem writes except stdout.
- No graph access — the bundle is the only source of truth about the
  feature, its TCs, and the schema. The worker does not touch `.product/`
  or `.dec/` directly.
- The output schema is strict — any deviation from `TcProposal` is a
  worker fault, not a low-confidence judgement.
- Single-shot — one Claude call per invocation (plus at most one retry on
  schema-validation failure). The worker does not loop or iterate.
- `sufficient` is a first-class outcome. The worker prefers `sufficient`
  over inventing redundant TCs when `existing_tcs` already cover the
  feature; the harness applies the [ADR-072](ADR-072) floor check as a
  precondition, but the worker is permitted to return `sufficient` if
  it judges the existing set substantively covers the axes (the matcher
  shape from [FT-048](FT-048)).
- `ProposedTc.runner_args` is required and non-empty. The brief (§2.9)
  fixes this: a TC without a wired runner is not ready for the
  implementer.

### Error handling

- Bundle missing required field → exit 2 with structured error.
- Bundle's `bundle_hash` malformed → exit 2.
- Claude API failure (network, auth, rate limit) → exit 3 with structured
  error; the harness will surface and may retry the *invocation*, not the
  LLM call.
- Schema validation failure on Claude response after one retry → exit 4
  with structured error including the failing response (for debug).
- `TcProposal.bundle_hash` ≠ input `bundle_hash` (impossible barring a
  worker bug) → exit 5.

### Boundaries

- **In scope.** Package layout (`workers/tc-author/`), `pyproject.toml`,
  the Pydantic models, the prompt template, the Anthropic call,
  structured-output validation, the `__main__` entry, ruff + pytest
  scaffolding, a unit test that exercises a mocked-Claude path against a
  synthetic bundle.
- **Out of scope.** Persisting the proposed TCs (the harness handles
  `product test new` + `product test runner` writes after the paired
  judge approves per [ADR-075](ADR-075)). The judge itself (lives in
  [FT-127](FT-127)). The harness-side matcher that triggers `sufficient`
  versus invocation (lives in [FT-131](FT-131)). The role-catalog
  authority declaration (lives alongside [FT-131](FT-131); the contract
  is fixed by [ADR-073](ADR-073) and [ADR-027](ADR-027)).

## Out of scope

- Persistence — the worker prints JSON, never writes graph artifacts or
  product-cli artifacts.
- Multi-feature batch proposals (one feature per call).
- Auto-acceptance — the harness, governed by [ADR-075](ADR-075), decides
  when to flip readiness bits; auto-flipping on tc-author output alone
  would skip the paired judge.
- Calling Claude more than twice per invocation (retry budget is 1).
- Graph access from inside the worker.
- Worker-side runner execution. The worker proposes runners; it does not
  run them. Runner execution is the implementer's downstream concern via
  `product verify`.
