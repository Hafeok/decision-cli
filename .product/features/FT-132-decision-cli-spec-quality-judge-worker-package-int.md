---
id: FT-132
title: 'decision-cli: spec-quality judge worker package — interprets feature_spec drafts against the originating request'
phase: 5
status: complete
depends-on:
- FT-129
- FT-127
- FT-021
- FT-067
- FT-055
adrs:
- ADR-073
- ADR-074
- ADR-075
- ADR-070
- ADR-072
tests:
- TC-316
- TC-317
- TC-318
- TC-319
domains: []
domains-acknowledged:
  ADR-071: 'ADR-071 governs in-process worker tool calls. FT-132 is an out-of-process Python subprocess: one Anthropic API call, stdout-only output, no filesystem access, no shell-out. Workspace-containment does not apply.'
---

## Description

Python worker package implementing the `spec-quality` role established by
[ADR-073](ADR-073): the interpretation half of the feature_spec authoring
pair. Takes a `SpecProposal` (the action artifact emitted by
[FT-129](FT-129)) plus the originating request/brief plus repo conventions
(the H2/H3 body schema from [FT-055](FT-055)/[ADR-047](ADR-047)), and emits
a `dec:QualityVerdict` ([ADR-074](ADR-074)) judging the proposal's fitness
for human acceptance.

Structurally identical to the slice-2 verifier worker ([FT-023](FT-023))
and the other quality judges ([FT-127](FT-127) tc-quality,
[FT-128](FT-128) vg-quality): stateless, single-shot, bundle-in /
verdict-out, no graph access, Pydantic strict I/O, structured-output
Claude call, retry budget 1, `python -m spec_quality --stdin` entry
point. The difference from the other judges is the rubric
(request-faithfulness + body-schema conformance) and the input class
(`SpecProposal`, not `TcProposal` or `GraphProposal`).

Per [ADR-075](ADR-075), an `approved` spec-quality verdict does NOT
auto-flip the readiness bit — feature_spec proposals are human-accept
(L3). The verdict is recorded; the proposal sits in `pending_review`; an
operator runs `dec drive accept --artifact <iri>` to flip the bit. The
judge therefore plays the role of *pre-screen for the human reviewer* —
catching schema violations, request-drift, and missing sections before
they reach human eyeballs.

Slice B per the brief (§5 build order). Depends on [FT-129](FT-129)
(the action half it judges) and [FT-127](FT-127) (the tc-quality judge
precedent).

## Functional Specification

### Inputs

- A Pydantic `SpecQualityInput` carrying the bundle:
  ```python
  class SpecQualityInput(BaseModel):
      spec_proposal: SpecProposalRecord          # the candidate from FT-129
      request: RequestRecord                     # originating brief
      body_schema: BodySchemaRecord              # H2/H3 contract from ADR-047
      related_features: list[FeatureRecord]      # for non-redundancy / scope-collision check
      central_adrs: list[AdrSummaryRecord]
      domain_registry: list[DomainRecord]
      rubric: SpecQualityRubricRecord
      authority: AuthorityRecord                 # ADR-027 declaration
      bundle_hash: str
  ```
  - `SpecProposalRecord` mirrors the `SpecProposal` schema emitted by
    [FT-129](FT-129).
  - `SpecQualityRubricRecord` carries the five rubric criteria below.
- An invocation: `python -m spec_quality --stdin`.
- Anthropic API key via env var.

### Outputs

- A Pydantic `QualityVerdict` printed to stdout as JSON (shape shared
  with [FT-127](FT-127) and [FT-128](FT-128)):
  ```python
  class QualityVerdict(BaseModel):
      verdict: Literal["approved", "rejected", "amendment-required"]
      rationale: str
      judges: str                                # IRI of the SpecProposal
      against: list[str]                         # [request IRI]
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
   `SpecQualityInput`.
2. Construct the prompt template with seven sections:
   - **Goal.** "Judge the proposed feature_spec for fitness as a human
     reviewer's bundle. The spec must conform to the H2/H3 schema, be
     faithful to the request, name an explicit scope and out-of-scope,
     and not collide with adjacent features."
   - **Request.** The originating `request` verbatim.
   - **Proposal.** The `spec_proposal` to judge: body, proposed
     depends-on, proposed ADRs, proposed domains, rationale.
   - **Body schema.** The required H2 sections (Description, Functional
     Specification, Out of scope) and Functional Specification H3
     subsections (Inputs, Outputs, State, Behaviour, Invariants, Error
     handling, Boundaries) from [ADR-047](ADR-047) /
     [FT-055](FT-055).
   - **Related features.** For the non-redundancy / scope-collision
     check.
   - **Central ADRs + domain registry.** For domain and ADR linkage
     soundness.
   - **Rubric.** The five criteria below, each with its passing
     threshold.
3. Call Claude with structured-output constraint matching
   `QualityVerdict`. Retry budget 1 on schema-validation failure.
4. Validate the response shape; enforce [ADR-018](ADR-018) /
   [ADR-074](ADR-074) constraints (rationale ≥ 20 chars,
   `rejected`/`amendment-required` cite ≥ 1 violated reference,
   `amendment-required` carries `amendment_guidance`).
5. Echo `bundle_hash` for harness pairing.
6. Print verdict JSON to stdout; exit 0.

### Rubric (the five criteria)

1. **Schema-conforming.** Body contains every required H2 section AND
   every Functional Specification H3 subsection per
   [ADR-047](ADR-047) / [FT-055](FT-055)'s `body-completeness` check.
2. **Request-faithful.** Every behaviour or boundary asserted in the
   spec maps to a clause in the originating request; no hallucinated
   scope, no missing intent.
3. **Bounded.** The `Out of scope` section is non-empty and lists at
   least one substantive boundary (not just a tautological "not in
   scope: anything else").
4. **Non-colliding.** Does not duplicate or contradict the scope of a
   `related_feature`. Boundary collisions with adjacent features are
   flagged in the rationale.
5. **Linkage-sound.** `proposed_depends_on` IDs exist; `proposed_adrs`
   IDs exist; `proposed_domains` are members of the `domain_registry`.

A proposal passes (`approved`) only if every rubric criterion is met.
For `gap`-kind SpecProposals, the rubric instead asks: is the
`missing_information` list a defensible enumeration of what the brief
under-specifies, or is it the spec-author deferring authoring it could
have completed?

### Invariants

- Stateless — no module-level state, no filesystem writes except stdout.
- No graph access — the bundle is the only source of truth.
- Output schema is strict — [ADR-074](ADR-074) shape adherence enforced
  at the worker boundary.
- Single-shot — one Claude call per invocation (plus at most one retry).
- The judge does not modify the proposal. It judges. Modification
  belongs to the next dispatch of [FT-129](FT-129) under the amendment
  loop.
- The judge does not consult the live product graph for the
  request-faithfulness check — the request is the bundle's
  `request.body`, period. Workers never reach into the orchestration
  store.

### Error handling

Identical exit-code mapping to [FT-127](FT-127) and [FT-128](FT-128):
- Bundle malformed / missing field → exit 2.
- Bundle's `bundle_hash` malformed → exit 2.
- Claude API failure → exit 3.
- Schema validation failure after one retry → exit 4 with response.
- `QualityVerdict.bundle_hash` ≠ input → exit 5.

### Boundaries

- **In scope.** Package layout (`workers/spec-quality/`), Pydantic
  models, the rubric-driven prompt template, the Anthropic call,
  structured-output validation, `__main__` entry, ruff + pytest
  scaffolding, a unit test per verdict kind against mocked Claude.
- **Out of scope.** Persisting the verdict (lives in the harness via the
  [ADR-074](ADR-074) SHACL chokepoint). The `DispatchGroup` lifecycle
  (inherited from [FT-021](FT-021)). The human-acceptance flow (a
  separate Slice B CLI feature). The amendment loop (handled by the
  harness invoking [FT-129](FT-129) again with `amendment_guidance` in
  the bundle).

## Out of scope

- Persistence — the worker prints JSON, never writes verdicts.
- Multi-proposal batch verdicts (one proposal per call).
- Auto-acceptance (governed by [ADR-075](ADR-075) — spec verdicts are
  human-accept, L3).
- Worker-side product-graph queries. The judge reads the bundle; it
  does not query the graph.
- Graduating spec verdicts to L4 autonomy (see [ADR-075](ADR-075)
  §"Future graduation"; criteria are data-driven, not role-consolidation
  driven, per [ADR-073](ADR-073)'s "Why four judge roles" decision).
