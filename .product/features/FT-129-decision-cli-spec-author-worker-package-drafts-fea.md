---
id: FT-129
title: 'decision-cli: spec-author worker package — drafts feature_spec bodies from a request brief'
phase: 5
status: planned
depends-on:
- FT-131
- FT-127
- FT-021
- FT-067
- FT-055
adrs:
- ADR-073
- ADR-074
- ADR-075
- ADR-047
- ADR-070
- ADR-072
tests:
- TC-298
- TC-299
- TC-300
- TC-301
domains: []
domains-acknowledged:
  ADR-071: 'ADR-071 governs in-process worker tool calls. FT-129 is an out-of-process Python subprocess: one Anthropic API call, stdout-only output, no filesystem access, no shell-out. Workspace-containment does not apply.'
---

## Description

Python worker package implementing the `spec-author` role established by
[ADR-073](ADR-073): the action half of the feature_spec authoring pair.
Takes an originating request/brief plus repo conventions (the H2/H3 body
structure from [FT-055](FT-055)/[ADR-047](ADR-047)) plus existing related
feature_specs, and returns a `SpecProposal` containing the proposed
feature_spec body (H2 sections per the body-completeness contract, H3
subsections for the Functional Specification).

This is the **highest trust boundary** in the chain (brief §4B). A
thinly-authored feature_spec produces code that passes a thinly-authored
TC; defects ripple downstream through many iterations before they surface.
Per [ADR-075](ADR-075), the paired judge's `approved` verdict on a
`SpecProposal` does NOT auto-flip the readiness bit — the proposal sits in
`pending_review` and a human operator accepts via `dec drive accept`. The
worker is honest about what it can and cannot judge: when the brief
under-specifies (no clear scope, contradictory constraints, missing
boundary), the worker returns a `Gap` outcome rather than padding.

Slice B per the brief (§5 build order). Depends on the Slice A capstone
([FT-131](FT-131)) being complete so the planner can dispatch this worker
with the right bundle shape.

Mirrors the [FT-048](FT-048) / [FT-126](FT-126) package contract exactly:
stateless, single-shot, bundle-in / artifact-out, no graph access,
Pydantic strict I/O, retry budget 1, `python -m spec_author --stdin`
entry point.

## Functional Specification

### Inputs

- A Pydantic `SpecAuthorInput` carrying the bundle:
  ```python
  class SpecAuthorInput(BaseModel):
      request: RequestRecord                     # originating brief / request
      feature_id_hint: Optional[str]             # if a placeholder ID was minted
      body_schema: BodySchemaRecord              # H2/H3 contract from ADR-047
      related_features: list[FeatureRecord]      # nearby specs by domain / depends-on
      central_adrs: list[AdrSummaryRecord]       # graph-central + cross-cutting
      domain_registry: list[DomainRecord]        # from product.toml
      authority: AuthorityRecord                 # ADR-027 declaration
      bundle_hash: str
  ```
  - `RequestRecord = { id, title, body, source }` — the structured form
    of the originating brief (typically minted by `product request apply`
    earlier in the chain).
  - `BodySchemaRecord` carries the required H2 sections
    (Description, Functional Specification, Out of scope) and the
    Functional Specification H3 subsections (Inputs, Outputs, State,
    Behaviour, Invariants, Error handling, Boundaries) from
    [ADR-047](ADR-047) / `product.toml` `[features]`.
- An invocation: `python -m spec_author --stdin`.
- Anthropic API key via env var.

### Outputs

- A Pydantic `SpecProposal` printed to stdout as JSON:
  ```python
  class SpecProposal(BaseModel):
      kind: Literal["new", "gap"]
      bundle_hash: str
      new: Optional[NewSpecProposal] = None
      gap: Optional[GapProposal] = None

  class NewSpecProposal(BaseModel):
      title: str
      body: str                                  # full markdown, H2/H3 conforming
      proposed_depends_on: list[str]             # FT-NNN ids
      proposed_adrs: list[str]                   # ADR-NNN ids the spec links
      proposed_domains: list[str]                # from domain_registry
      rationale: str

  class GapProposal(BaseModel):
      missing_information: list[str]             # what the brief should clarify
      reason: str                                # why the worker cannot author
  ```
- Exit 0 on a structured proposal returned; non-zero on infrastructure
  failure.

### State

- None. Stateless; bundle in, proposal out. No graph access; no disk
  writes beyond stdout.

### Behaviour

1. Parse the input bundle from stdin; validate against `SpecAuthorInput`.
2. Construct the prompt template with seven sections:
   - **Goal.** "Author a feature_spec that addresses the request. The
     body must conform to the H2/H3 schema. The spec must be testable
     (every required behaviour maps to at least one observable
     condition) and bounded (the Out of scope section is non-empty)."
   - **Request.** The `request` body verbatim.
   - **Body schema.** The required H2 sections and Functional
     Specification H3 subsections; each section's expected content.
   - **Related features.** A digest of `related_features` (titles +
     Description + Boundaries from each) so the spec doesn't duplicate
     scope.
   - **Central ADRs.** The `central_adrs` (graph-central per
     `product graph central`) — the foundational decisions the spec
     must respect.
   - **Domain registry.** Allowed `domains:` values for the spec
     frontmatter.
   - **Authority.** The role's [ADR-027](ADR-027) declaration. The
     worker is instructed: if the request leaves architectural
     decisions ambiguous, emit a `gap` proposal rather than inventing
     decisions that belong in ADRs.
3. Call Claude with structured-output constraint matching `SpecProposal`.
   Retry budget 1.
4. Validate the response: for `new`, verify the body string contains all
   required H2 sections and Functional Specification H3 subsections per
   `body_schema`; on failure retry once with a diagnostic; if it still
   fails, fall back to `gap` with reason "could not produce
   schema-conformant body."
5. Echo `bundle_hash` for harness pairing.
6. Print proposal JSON to stdout; exit 0.

### Invariants

- Stateless — no module-level state, no filesystem writes except stdout.
- No graph access — the bundle is the only source of truth.
- Output schema is strict.
- Single-shot — one Claude call per invocation (plus at most one retry).
- `gap` is a first-class outcome. The worker prefers `gap` over
  inventing scope.
- The proposed body MUST conform to [ADR-047](ADR-047)'s H2/H3 schema;
  the worker is validated against the body-completeness check
  ([FT-055](FT-055)) before exit.

### Error handling

Identical exit-code mapping to [FT-126](FT-126):
- Bundle missing / malformed → exit 2.
- Claude API failure → exit 3.
- Schema validation failure after one retry → exit 4 with response.
- `SpecProposal.bundle_hash` ≠ input → exit 5.

### Boundaries

- **In scope.** Package layout (`workers/spec-author/`), Pydantic models,
  the prompt template, the Anthropic call, structured-output validation,
  `__main__` entry, ruff + pytest scaffolding, a unit test per proposal
  kind against mocked Claude.
- **Out of scope.** Persisting the proposed spec (the harness handles
  `product feature new` + `product body update` writes only AFTER human
  acceptance per [ADR-075](ADR-075)). The paired judge ([FT-132](FT-132)
  spec-quality — one of four dedicated judges per [ADR-073](ADR-073)
  §"Why four judge roles, not fewer"). The acceptance autonomy decision
  (inherited from [ADR-075](ADR-075) — spec verdicts are human-accept).

## Out of scope

- Persistence — the worker prints JSON, never writes specs.
- Multi-feature batch proposals (one request per call).
- Authoring TCs alongside the spec (`tc-author` ([FT-126](FT-126))
  handles TCs; the spec-author may *suggest* what the TCs should cover
  in the body's "Description" section but does not author them).
- Authoring ADRs (`adr-author` ([FT-130](FT-130)) handles ADRs;
  spec-author lists `proposed_adrs` for existing ADRs the spec links to
  and may emit a `gap` if a missing ADR blocks the spec).
- Graduating spec verdicts to L4 autonomy (see [ADR-075](ADR-075)
  §"Future graduation").
