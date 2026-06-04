---
id: FT-133
title: 'decision-cli: adr-quality judge worker package — interprets ADR drafts and acknowledgements against the preflight gap'
phase: 5
status: complete
depends-on:
- FT-130
- FT-132
- FT-127
- FT-021
- FT-067
adrs:
- ADR-073
- ADR-074
- ADR-075
- ADR-070
- ADR-072
tests:
- TC-320
- TC-321
- TC-322
- TC-323
domains: []
domains-acknowledged:
  ADR-071: 'ADR-071 governs in-process worker tool calls. FT-133 is an out-of-process Python subprocess: one Anthropic API call, stdout-only output, no filesystem access, no shell-out. Workspace-containment does not apply.'
---

## Description

Python worker package implementing the `adr-quality` role established by
[ADR-073](ADR-073): the interpretation half of the ADR authoring pair.
Takes an `AdrProposal` (the action artifact emitted by [FT-130](FT-130))
plus the preflight gap it addresses plus the feature_spec plus the
central ADRs, and emits a `dec:QualityVerdict` ([ADR-074](ADR-074))
judging the proposal's fitness for human acceptance.

Structurally identical to [FT-127](FT-127) (tc-quality),
[FT-128](FT-128) (vg-quality), and [FT-132](FT-132) (spec-quality):
stateless, single-shot, bundle-in / verdict-out, no graph access,
Pydantic strict I/O, structured-output Claude call, retry budget 1,
`python -m adr_quality --stdin` entry point. The difference from the
other judges is the rubric (gap-closure soundness + no-bare-acknowledgement
rule) and the input class (`AdrProposal`, not `SpecProposal` or
`TcProposal` or `GraphProposal`).

Per [ADR-075](ADR-075), an `approved` adr-quality verdict does NOT
auto-flip the readiness bit — ADR proposals (whether new ADRs or
acknowledgements) are human-accept (L3). The verdict is recorded; the
proposal sits in `pending_review`; an operator runs `dec drive accept`
to flip the bit. The judge plays the role of *pre-screen for the
human reviewer* — catching bare acknowledgements, scope mismatches,
and ADR H2 violations before they reach human eyeballs.

Slice B per the brief (§5 build order). Depends on [FT-130](FT-130)
(the action half it judges), [FT-132](FT-132) (sibling judge — shares
the prose-judge prompt infrastructure), and [FT-127](FT-127)
(judge precedent).

## Functional Specification

### Inputs

- A Pydantic `AdrQualityInput` carrying the bundle:
  ```python
  class AdrQualityInput(BaseModel):
      adr_proposal: AdrProposalRecord            # the candidate from FT-130
      feature_id: str
      feature_spec: str
      preflight_gap: PreflightGapRecord          # the gap the proposal addresses
      central_adrs: list[AdrSummaryRecord]
      adr_body_schema: BodySchemaRecord          # H2 contract for ADRs
      domain_registry: list[DomainRecord]
      rubric: AdrQualityRubricRecord
      authority: AuthorityRecord                 # ADR-027 declaration
      bundle_hash: str
  ```
  - `AdrProposalRecord` mirrors the `AdrProposal` schema emitted by
    [FT-130](FT-130) (`new` / `acknowledgement` / `gap`).
  - `PreflightGapRecord` mirrors the shape from [FT-130](FT-130).
- An invocation: `python -m adr_quality --stdin`.
- Anthropic API key via env var.

### Outputs

- A Pydantic `QualityVerdict` printed to stdout as JSON (shape shared
  with [FT-127](FT-127), [FT-128](FT-128), [FT-132](FT-132)):
  ```python
  class QualityVerdict(BaseModel):
      verdict: Literal["approved", "rejected", "amendment-required"]
      rationale: str
      judges: str                                # IRI of the AdrProposal
      against: list[str]                         # [preflight_gap IRI, feature_spec IRI]
      violates: list[str] = []
      amendment_guidance: Optional[str] = None
      bundle_hash: str
  ```
- Exit 0 on a structured verdict returned; non-zero on infrastructure
  failure.

### State

- None. Stateless; bundle in, verdict out. No graph access; no disk
  writes beyond stdout.

### Behaviour

1. Parse the input bundle from stdin; validate against
   `AdrQualityInput`.
2. Construct the prompt template with seven sections:
   - **Goal.** "Judge the proposed ADR (or acknowledgement) for fitness
     as a human reviewer's bundle. A NEW ADR must conform to the H2
     schema and soundly close the preflight gap. An ACKNOWLEDGEMENT
     must reference an existing ADR that genuinely governs the
     feature, with reasoning ≥ 40 chars."
   - **Feature.** Embedded `feature_spec` body.
   - **Gap.** The `preflight_gap` payload.
   - **Proposal.** The `adr_proposal` to judge: kind + payload.
   - **ADR body schema.** Required H2 sections (Context, Decision,
     Rejected alternatives, Consequences, Status).
   - **Central ADRs + domain registry.** For scope soundness and
     domain validity checks.
   - **Rubric.** The five criteria below.
3. Call Claude with structured-output constraint matching
   `QualityVerdict`. Retry budget 1.
4. Validate the response shape; enforce [ADR-018](ADR-018) /
   [ADR-074](ADR-074) constraints.
5. Echo `bundle_hash` for harness pairing.
6. Print verdict JSON to stdout; exit 0.

### Rubric (the five criteria)

Applied conditional on the proposal `kind`:

For **`new` proposals** (a net-new ADR):

1. **Schema-conforming.** Body contains every required H2 section
   (Context, Decision, Rejected alternatives, Consequences, Status).
2. **Gap-closing.** The proposed Decision section materially addresses
   the `preflight_gap`. A gap citing ADR-XXX is closed by either
   (a) extending ADR-XXX (caught at the worker — adr-author returns
   amendment, not new) or (b) authoring a NEW ADR that explicitly
   relates to ADR-XXX in its Context section.
3. **Scope-correct.** The proposed `scope` field
   (`cross-cutting` / `platform` / `domain` / `feature-specific`)
   matches the gap kind: a `cross-cutting` ADR for a cross-cutting
   gap, a `feature-specific` ADR for a per-feature gap, etc.
4. **Domain-valid.** Every `proposed_domains` entry is a member of the
   `domain_registry`.
5. **Alternatives-noted.** The Rejected alternatives section names at
   least two substantive alternatives with rationale.

For **`acknowledgement` proposals**:

1. **Reasoning length floor.** `reasoning` is at least 40 characters
   (the brief §4B no-bare-ack rule, also enforced at the action
   worker boundary in [FT-130](FT-130)).
2. **References existing.** `acknowledges` field is an ADR-NNN id that
   exists in the central ADR catalog.
3. **Reasoning relevance.** The reasoning materially explains why the
   referenced existing ADR governs the feature — not a generic
   "applies workspace-wide" boilerplate.
4. **Gap-matching.** The `preflight_gap` is the gap the acknowledgement
   addresses; the acknowledgement is not silently retargeting a
   different gap.
5. **Not-better-as-new.** The reasoning is not a wishful "this existing
   ADR almost fits"; if the gap genuinely needs a new ADR, the
   verdict is `amendment-required` redirecting to a `new` proposal.

For **`gap`-kind AdrProposals**, the rubric asks: is the
`missing_information` list defensible, or could adr-author have
produced either a `new` or an `acknowledgement` from what it had?

### Invariants

- Stateless — no module-level state, no filesystem writes except stdout.
- No graph access — the bundle is the only source of truth.
- Output schema is strict — [ADR-074](ADR-074) adherence at the worker
  boundary; SHACL is the durable backstop.
- Single-shot — one Claude call per invocation (plus at most one retry).
- The judge does not modify the proposal. Modification belongs to the
  next dispatch of [FT-130](FT-130) under the amendment loop.
- Bare acknowledgements (received from a hypothetical broken
  adr-author) are categorically rejected — [FT-130](FT-130) blocks
  these at its boundary, but the judge applies the same rule as
  defense in depth.

### Error handling

Identical exit-code mapping to [FT-127](FT-127), [FT-128](FT-128),
[FT-132](FT-132):
- Bundle malformed / missing field → exit 2.
- Bundle's `bundle_hash` malformed → exit 2.
- Claude API failure → exit 3.
- Schema validation failure after one retry → exit 4 with response.
- `QualityVerdict.bundle_hash` ≠ input → exit 5.

### Boundaries

- **In scope.** Package layout (`workers/adr-quality/`), Pydantic
  models, the rubric-driven prompt template, the Anthropic call,
  structured-output validation, `__main__` entry, ruff + pytest
  scaffolding, a unit test per verdict kind against mocked Claude.
- **Out of scope.** Persisting the verdict (lives in the harness via the
  [ADR-074](ADR-074) SHACL chokepoint). The `DispatchGroup` lifecycle
  (inherited from [FT-021](FT-021)). The human-acceptance flow (a
  separate Slice B CLI feature). The amendment loop (handled by the
  harness invoking [FT-130](FT-130) again with `amendment_guidance`).

## Out of scope

- Persistence — the worker prints JSON, never writes verdicts.
- Multi-proposal batch verdicts (one proposal per call).
- Auto-acceptance (governed by [ADR-075](ADR-075) — ADR verdicts are
  human-accept, L3).
- Worker-side product-graph queries. The judge reads the bundle; it
  does not query the graph.
- Adjudicating between competing new-ADR proposals (the harness
  re-dispatches on amendment; the judge sees one proposal at a time).
- Graduating ADR verdicts to L4 autonomy (see [ADR-075](ADR-075)
  §"Future graduation").
