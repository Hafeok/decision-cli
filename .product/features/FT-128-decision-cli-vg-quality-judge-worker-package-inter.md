---
id: FT-128
title: 'decision-cli: vg-quality judge worker package — interprets VerificationGraph proposals against covered TCs'
phase: 4
status: planned
depends-on:
- FT-048
- FT-021
- FT-020
- FT-019
- FT-067
adrs:
- ADR-073
- ADR-074
- ADR-075
- ADR-030
- ADR-070
- ADR-072
tests:
- TC-294
- TC-295
- TC-296
- TC-297
domains: []
domains-acknowledged:
  ADR-071: 'ADR-071 governs in-process worker tool calls. FT-128 is an out-of-process Python subprocess: one Anthropic API call, stdout-only output, no filesystem access, no shell-out. Workspace-containment does not apply.'
---

## Description

Python worker package implementing the `vg-quality` role established by
[ADR-073](ADR-073): the interpretation half of the verify-graph authoring
pair. Takes a `GraphProposal` (the action artifact emitted by the existing
verify-graph-author worker [FT-048](FT-048)) plus the TC(s) it claims to
cover and the target environment, and emits a `dec:QualityVerdict`
([ADR-074](ADR-074)) judging the proposal's fitness for the graph-runner to
execute.

Structurally identical to [FT-127](FT-127) (the TC-quality judge) and the
slice-2 verifier worker ([FT-023](FT-023)): stateless, single-shot, bundle-in
/ verdict-out, no graph access, Pydantic strict I/O, structured-output Claude
call, retry budget 1, `python -m vg_quality --stdin` entry point. The
difference from FT-127 is the rubric (graph minimality and evidence soundness)
and the input class (`GraphProposal`, not `TcProposal`).

The judge feeds the `vgs_ready` dimension in the planner ([FT-131](FT-131))
— the quality counterpart to today's structural `vgs_accepted`. Together
with [FT-127](FT-127) it lands Slice A's two judge roles per the brief
(§5 build order).

## Functional Specification

### Inputs

- A Pydantic `VgQualityInput` carrying the bundle:
  ```python
  class VgQualityInput(BaseModel):
      feature_id: str
      feature_spec: str                          # full markdown body
      graph_proposal: GraphProposalRecord        # the candidate from FT-048
      covered_tcs: list[TcRecord]                # TCs the proposal claims to cover
      target_environment: EnvRecord              # ADR-030 env shape
      step_vocabulary: list[StepKindRecord]      # FT-036 seed kinds + allowed_ops
      existing_graphs: list[ExistingGraphRecord] # for non-redundancy check
      rubric: VgQualityRubricRecord
      authority: AuthorityRecord                 # ADR-027 declaration
      bundle_hash: str
  ```
  - `GraphProposalRecord` mirrors the `GraphProposal` schema emitted by
    [FT-048](FT-048) (`match` / `new` / `gap`).
  - `EnvRecord = { id, env_type, safety_class, allowed_ops, endpoint? }`
    per [ADR-030](ADR-030).
  - `VgQualityRubricRecord` is the controlled vocabulary the judge scores
    against (see "Rubric" below).
- An invocation: `python -m vg_quality --stdin`.
- Anthropic API key via env var.

### Outputs

- A Pydantic `QualityVerdict` printed to stdout as JSON (shape shared with
  [FT-127](FT-127)):
  ```python
  class QualityVerdict(BaseModel):
      verdict: Literal["approved", "rejected", "amendment-required"]
      rationale: str
      judges: str                                # IRI of the GraphProposal
      against: list[str]                         # TC IRIs + env IRI
      violates: list[str] = []
      amendment_guidance: Optional[str] = None
      bundle_hash: str
  ```
- Exit 0 on a structured verdict returned; non-zero on infrastructure
  failure.

### State

- None. Stateless; bundle in, verdict out. No graph access; no disk writes
  beyond stdout.

### Behaviour

1. Parse the input bundle from stdin; validate against `VgQualityInput`.
2. Construct the prompt with six sections:
   - **Goal.** "Judge the proposed verification graph for fitness as
     graph-runner input. The graph passes only if it demonstrates every
     `covered_tc` in the target environment using a minimal step set."
   - **Feature.** Embedded `feature_spec` body.
   - **TCs under coverage.** The `covered_tcs` with their bodies.
   - **Environment.** Target env (env_type, safety_class, allowed_ops).
   - **Proposal.** The `GraphProposal` to judge: for `match` kind, the
     match graph_id and its existing steps; for `new` kind, the proposed
     steps and `provides_evidence_for` mappings; for `gap` kind, the
     proposal's `uncovered_tcs` and `reason`.
   - **Rubric.** The four criteria below, each with its passing
     threshold.
3. Call Claude with structured-output constraint matching `QualityVerdict`.
   Retry budget 1.
4. Validate the response shape; enforce [ADR-018](ADR-018) constraints
   (rationale ≥ 20 chars, conditional `violates`, conditional
   `amendment_guidance`).
5. Echo `bundle_hash` for harness pairing.
6. Print verdict JSON to stdout; exit 0.

### Rubric (the four criteria)

1. **Steps demonstrate the TC.** Each `covered_tc` has at least one step
   whose `provides_evidence_for` references it AND whose `step_type` +
   `fields` constitute a believable demonstration of the TC's claim
   (per [ADR-030](ADR-030) §"Coverage is structural").
2. **`requiredOps ⊆ allowedOps`, minimally.** The union of every step's
   `required_ops` is a subset of the env's `allowed_ops`, AND the subset
   relationship is tight: no step uses an op that no other step uses, no
   redundant op claims. The brief (§4B) flags this as "minimal not just
   legal."
3. **Environment appropriate.** The chosen env's `safety_class` matches
   the action's destructiveness: read-only TCs don't claim destructive
   envs, destructive TCs aren't run in read-only envs.
4. **Evidence mapping sound.** `provides_evidence_for` on each step lists
   exactly the TCs the step's `fields` actually evidence — no
   over-claiming, no under-claiming. Steps with empty
   `provides_evidence_for` are justified in the proposal's rationale
   (setup / capture steps).

For `match` kind, the rubric scores the matched graph (not a new
authored set) — the judge inspects the existing graph's steps and
applies the same four criteria to the matched evidence.

For `gap` kind, the rubric instead asks: is the `gap.reason` valid?
i.e. does the step vocabulary genuinely lack the operations the TCs
demand, OR did the action worker miss a viable composition? An
`approved` `gap` verdict routes the gap as feedback ([ADR-022](ADR-022));
a `rejected` `gap` verdict re-dispatches FT-048 with the rejection as
amendment guidance.

### Invariants

- Stateless — no module-level state, no filesystem writes except stdout.
- No graph access — the bundle is the only source of truth.
- Output schema is strict — [ADR-074](ADR-074) shape adherence enforced
  at the worker boundary.
- Single-shot — one Claude call per invocation (plus at most one retry).
- The judge does not modify the proposal. It judges. Modification belongs
  to the next dispatch of [FT-048](FT-048) under the amendment loop.

### Error handling

Identical exit-code mapping to [FT-127](FT-127):
- Bundle malformed / missing field → exit 2.
- Bundle's `bundle_hash` malformed → exit 2.
- Claude API failure → exit 3.
- Schema validation failure after one retry → exit 4 with response.
- `QualityVerdict.bundle_hash` ≠ input → exit 5.

### Boundaries

- **In scope.** Package layout (`workers/vg-quality/`), Pydantic models,
  the rubric-driven prompt template, the Anthropic call, structured-output
  validation, `__main__` entry, ruff + pytest scaffolding, a unit test
  per verdict kind against a mocked Claude.
- **Out of scope.** Persisting the verdict (harness handles via the
  [ADR-074](ADR-074) SHACL chokepoint). The `DispatchGroup` lifecycle
  (inherited from [FT-021](FT-021)). Auto-acceptance autonomy (governed
  by [ADR-075](ADR-075) — VG verdicts auto-flip per the fitness
  function). The amendment loop (handled by the harness invoking
  [FT-048](FT-048) again with `amendment_guidance`).

## Out of scope

- Persistence — the worker prints JSON, never writes verdicts.
- Multi-graph batch verdicts (one proposal per call).
- Multi-environment composite verdicts (one env per call, mirroring
  [FT-048](FT-048) §"One graph per environment per call").
- Worker-side graph execution. The judge reads the proposal; it does not
  run steps. Execution is the graph-runner's downstream concern.
- Graduating VG verdicts to L5 autonomy (see [ADR-075](ADR-075)
  §"Future graduation").
