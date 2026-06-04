---
id: FT-127
title: 'decision-cli: tc-quality judge worker package — interprets TC drafts against feature_spec'
phase: 4
status: complete
depends-on:
- FT-126
- FT-021
- FT-020
- FT-019
- FT-067
adrs:
- ADR-073
- ADR-074
- ADR-075
- ADR-027
- ADR-070
- ADR-072
tests:
- TC-290
- TC-291
- TC-292
- TC-293
domains: []
domains-acknowledged:
  ADR-071: ADR-071 governs in-process worker tool calls (workspace containment + secrets blocking inside an in-process agentic loop per FT-123). FT-127 is an out-of-process Python subprocess that makes a single Anthropic API call via the SDK and writes only to stdout — no in-process tool surface, no filesystem access, no shell-out. Workspace-containment does not apply.
---

## Description

Python worker package implementing the `tc-quality` role established by
[ADR-073](ADR-073): the interpretation half of the TC authoring pair. Takes
a `TcProposal` (the action artifact emitted by [FT-126](FT-126)) plus the
feature_spec the TC must serve, and emits a `dec:QualityVerdict`
([ADR-074](ADR-074)) judging the proposal's fitness for an implementer to
consume.

Structurally identical to the slice-2 verifier worker ([FT-023](FT-023)):
stateless, single-shot, bundle-in / verdict-out, no graph access, Pydantic
strict I/O, structured-output Claude call, retry budget 1,
`python -m tc_quality --stdin` entry point. The difference from the
verifier is the rubric (TC fitness, not code-satisfies-TC) and the verdict
class (`dec:QualityVerdict`, not `dec:VerificationVerdict`).

The worker is the first non-code interpretation session in the system. It
exercises [ADR-017](ADR-017) (paired interpretation) and [ADR-018](ADR-018)
(verdict shape) on the new `dec:QualityVerdict` class. The fitness function
that watches its agreement rate ([ADR-075](ADR-075)) consumes the output of
this judge to decide auto-accept policy on TCs over time.

## Functional Specification

### Inputs

- A Pydantic `TcQualityInput` carrying the bundle:
  ```python
  class TcQualityInput(BaseModel):
      feature_id: str
      feature_spec: str                          # full markdown body
      tc_proposal: TcProposalRecord              # the candidate from FT-126
      existing_tcs: list[TcRecord]               # for non-redundancy check
      rubric: TcQualityRubricRecord
      authority: AuthorityRecord                 # ADR-027 declaration
      bundle_hash: str
  ```
  - `TcProposalRecord` mirrors the `TcProposal` schema emitted by
    [FT-126](FT-126) — the proposal under judgment.
  - `TcQualityRubricRecord` is the controlled vocabulary the judge scores
    against (see "Rubric" below).
  - `AuthorityRecord` is the role's [ADR-027](ADR-027) authority
    declaration: `mayDecide`, `mustEscalate`, `escalateVia` lists.
- An invocation: `python -m tc_quality --stdin` (entry point identical in
  shape to [FT-023](FT-023) and [FT-048](FT-048)).
- Anthropic API key via env var.

### Outputs

- A Pydantic `QualityVerdict` printed to stdout as JSON:
  ```python
  class QualityVerdict(BaseModel):
      verdict: Literal["approved", "rejected", "amendment-required"]
      rationale: str                             # >= 20 chars, ADR-018 floor
      judges: str                                # IRI of the TcProposal under judgment
      against: list[str]                         # [feature_spec IRI]
      violates: list[str] = []                   # rejected/amendment-required only
      amendment_guidance: Optional[str] = None   # amendment-required only
      bundle_hash: str                           # echo
  ```
- Exit 0 on a structured verdict returned (regardless of `verdict`).
- Exit non-zero on infrastructure failure.

### State

- None. Stateless; bundle in, verdict out. No graph access; no disk writes
  beyond stdout. The harness — not this worker — persists the
  `dec:QualityVerdict` artifact after `DispatchGroup` lifecycle transitions
  (per [FT-021](FT-021)).

### Behaviour

1. Parse the input bundle from stdin; validate against `TcQualityInput`.
2. Construct the prompt template with five sections:
   - **Goal.** "Judge each proposed TC for fitness as an implementer
     bundle. Score against the rubric. If the proposal is approved, every
     TC in it must be clear, testable, non-redundant, faithful to the
     spec, and have a wireable runner."
   - **Feature.** Embedded `feature_spec` body.
   - **Proposal.** The `tc_proposal` to judge, with each `ProposedTc`'s
     body, runner, runner_args, runner_timeout.
   - **Existing TCs.** For the non-redundancy check.
   - **Rubric.** The five criteria below, each with its passing threshold.
   - **Authority.** The judge's `mayDecide` / `mustEscalate` declaration
     ([ADR-027](ADR-027)). The judge is instructed to emit
     `amendment-required` with `amendment_guidance` on
     `mayDecide`-scoped issues (style, naming) and `rejected` on
     `mustEscalate`-scoped issues (the proposal violates the spec or the
     schema).
3. Call Claude with structured-output constraint matching
   `QualityVerdict`. Retry budget 1 on schema-validation failure.
4. Validate the response shape against `QualityVerdict`. The structured
   output enforces [ADR-018](ADR-018) constraints (rationale ≥ 20 chars,
   `rejected`/`amendment-required` cite ≥ 1 violated reference,
   `amendment-required` carries `amendment_guidance`).
5. Echo `bundle_hash` for harness pairing.
6. Print the verdict JSON to stdout; exit 0.

### Rubric (the five criteria)

Each `ProposedTc` is scored against five criteria. The judge emits a
single overall `verdict` per proposal; the rationale enumerates the
per-criterion scores.

1. **Clear.** Title and body unambiguous; the test's claim is stated
   declaratively.
2. **Testable.** A reader can construct a pass/fail oracle from the body
   (per [ADR-013](ADR-013) two-tier exit-code contract).
3. **Non-redundant.** Distinct from every other proposed TC AND from
   every `existing_tc` (semantic redundancy, not literal text).
4. **Faithful to spec.** The behaviour asserted is in the feature_spec
   (not a hallucinated requirement) and the spec asserts it (not just
   alludes to it).
5. **Runner-wireable.** `runner` is in the controlled vocabulary;
   `runner_args` is non-empty and matches the runner kind's
   `args_pattern`; `runner_timeout` is well-formed.

A proposal passes (`approved`) only if every TC clears all five criteria
for proposals of kind `augment` / `new`. For kind `sufficient`, the
judge instead validates the `coverage_map` against the feature_spec and
existing TCs (does the claim hold that existing TCs cover the axes
declared?).

### Invariants

- Stateless — no module-level state, no filesystem writes except stdout.
- No graph access — the bundle is the only source of truth.
- Output schema is strict — adherence to the `dec:QualityVerdict` shape
  per [ADR-074](ADR-074) is enforced at the worker boundary; the
  StreamWriter SHACL is the durable backstop.
- Single-shot — one Claude call per invocation (plus at most one retry
  on schema-validation failure).
- `rejected` and `amendment-required` MUST cite ≥ 1 violated reference
  via `violates` (per [ADR-018](ADR-018) §"SHACL shape"); the structured
  output enforces this at LLM time, the SHACL enforces at write time.
- The judge does not modify the proposal. It judges. Modification belongs
  to the next dispatch of [FT-126](FT-126) (the amendment loop, per
  [FT-021](FT-021)).

### Error handling

- Bundle missing required field → exit 2.
- Bundle's `bundle_hash` malformed → exit 2.
- Claude API failure → exit 3.
- Schema validation failure after one retry → exit 4 with the failing
  response.
- `QualityVerdict.bundle_hash` ≠ input `bundle_hash` → exit 5.

### Boundaries

- **In scope.** Package layout (`workers/tc-quality/`), Pydantic models,
  the rubric-driven prompt template, the Anthropic call,
  structured-output validation, `__main__` entry, ruff + pytest
  scaffolding, a unit test that exercises a mocked-Claude path against a
  synthetic bundle for each verdict kind.
- **Out of scope.** Persisting the verdict (lives in the harness; the
  StreamWriter chokepoints the [ADR-074](ADR-074) SHACL). The
  `DispatchGroup` lifecycle (inherited from [FT-021](FT-021)). The
  acceptance autonomy decision (inherited from [ADR-075](ADR-075) — TC
  verdicts auto-flip the readiness bit, governed by the fitness
  function). The fitness function itself (a TC under [ADR-014](ADR-014)
  authored alongside [FT-131](FT-131)).

## Out of scope

- Persistence — the worker prints JSON, never writes verdicts to the
  store.
- Multi-proposal batch verdicts (one proposal per call).
- The amendment loop (handled by the harness invoking [FT-126](FT-126)
  again with `amendment_guidance` in the bundle).
- The verifier ([FT-023](FT-023)) is a sibling worker but a distinct
  role; this judge does not consume code or emit `VerificationVerdict`
  artifacts.
- Worker-side execution of the proposed TCs' runners. The judge reads
  the runner spec; it does not run the tests. Execution happens via
  `product verify` after the harness persists the TCs.
