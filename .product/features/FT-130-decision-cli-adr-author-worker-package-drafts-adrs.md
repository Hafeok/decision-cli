---
id: FT-130
title: 'decision-cli: adr-author worker package — drafts ADRs or acknowledgements for preflight gaps'
phase: 5
status: planned
depends-on:
- FT-129
- FT-127
- FT-067
- FT-021
- FT-104
adrs:
- ADR-073
- ADR-074
- ADR-075
- ADR-070
- ADR-072
tests:
- TC-302
- TC-303
- TC-304
- TC-305
domains: []
domains-acknowledged:
  ADR-071: 'ADR-071 governs in-process worker tool calls. FT-130 is an out-of-process Python subprocess: one Anthropic API call, stdout-only output, no filesystem access, no shell-out. Workspace-containment does not apply.'
---

## Description

Python worker package implementing the `adr-author` role established by
[ADR-073](ADR-073): the action half of the ADR authoring pair. Takes a
preflight gap (an unacknowledged cross-cutting ADR or domain) plus the
feature_spec that surfaced the gap plus the central ADRs, and returns
either a proposed `ADR` (the gap warrants a new decision) or an
`Acknowledgement` with reasoning (the existing ADR governs the feature
but the spec missed the link, or the gap is genuinely out of scope).

Bare acknowledgement is rejected per the domain-coverage rule —
[ADR-104](ADR-104) (default-acknowledge cross-cutting ADRs via
`product.toml`) handles trivial acknowledgements automatically; the
adr-author worker is only invoked for gaps that survive the default-ack
pass. Every acknowledgement this worker produces MUST carry a reasoning
string (the brief §4B is explicit).

Slice B per the brief (§5 build order). Depends on [FT-129](FT-129)
(sibling spec-author lands first) to share the body-schema prompt
infrastructure and on [FT-131](FT-131) (the planner that dispatches the
worker) being complete first.

Mirrors the [FT-048](FT-048) / [FT-126](FT-126) package contract exactly:
stateless, single-shot, bundle-in / artifact-out, no graph access,
Pydantic strict I/O, retry budget 1, `python -m adr_author --stdin`
entry point.

## Functional Specification

### Inputs

- A Pydantic `AdrAuthorInput` carrying the bundle:
  ```python
  class AdrAuthorInput(BaseModel):
      feature_id: str
      feature_spec: str                          # full markdown body
      preflight_gap: PreflightGapRecord          # the gap to address
      central_adrs: list[AdrSummaryRecord]       # graph-central + cross-cutting
      adr_body_schema: BodySchemaRecord          # H2 structure for ADRs
      domain_registry: list[DomainRecord]        # from product.toml
      authority: AuthorityRecord                 # ADR-027 declaration
      bundle_hash: str

  class PreflightGapRecord(BaseModel):
      kind: Literal["unacknowledged-adr", "uncovered-domain"]
      adr_id: Optional[str]                      # unacknowledged-adr only
      domain: Optional[str]                      # uncovered-domain only
      severity: Literal["warning", "error"]
      message: str                               # diagnostic from product preflight
  ```
- An invocation: `python -m adr_author --stdin`.
- Anthropic API key via env var.

### Outputs

- A Pydantic `AdrProposal` printed to stdout as JSON:
  ```python
  class AdrProposal(BaseModel):
      kind: Literal["new", "acknowledgement", "gap"]
      bundle_hash: str
      new: Optional[NewAdrProposal] = None
      acknowledgement: Optional[AcknowledgementProposal] = None
      gap: Optional[GapProposal] = None

  class NewAdrProposal(BaseModel):
      title: str
      body: str                                  # H2 conforming
      scope: Literal["cross-cutting", "platform", "domain", "feature-specific"]
      proposed_domains: list[str]
      addresses_gap: PreflightGapRecord
      rationale: str

  class AcknowledgementProposal(BaseModel):
      acknowledges: str                          # ADR-NNN or domain name
      target_feature: str                        # FT-NNN
      reasoning: str                             # MUST be non-empty (brief §4B)
      rationale: str

  class GapProposal(BaseModel):
      missing_information: list[str]
      reason: str
  ```
- Exit 0 on a structured proposal returned; non-zero on infrastructure
  failure.

### State

- None. Stateless; bundle in, proposal out. No graph access; no disk
  writes beyond stdout.

### Behaviour

1. Parse the input bundle from stdin; validate against `AdrAuthorInput`.
2. Construct the prompt template with seven sections:
   - **Goal.** "Address the preflight gap. Choose: author a NEW ADR if
     the gap warrants a new decision; AUTHOR an acknowledgement with
     reasoning if an existing ADR governs the feature and the
     acknowledgement closes the gap; emit GAP if the brief
     under-specifies the decision space."
   - **Feature.** Embedded `feature_spec` body.
   - **Gap.** The `preflight_gap` payload.
   - **Central ADRs.** `central_adrs` for context.
   - **Body schema.** The required H2 sections for an ADR (Context,
     Decision, Rejected alternatives, Consequences, Status).
   - **Domain registry.** Allowed `domains:` values.
   - **Authority.** The role's [ADR-027](ADR-027) declaration. The
     worker is instructed: bare acknowledgement (no reasoning) is
     forbidden; if neither a new ADR nor a reasoned acknowledgement is
     defensible, emit `gap`.
3. Call Claude with structured-output constraint matching `AdrProposal`.
   Retry budget 1.
4. Validate the response. For `new`, verify the body string contains all
   required H2 sections per `adr_body_schema`. For `acknowledgement`,
   verify `reasoning` is non-empty AND ≥ 40 characters (brief §4B's
   "non-bare" floor). On failure retry once; on persistent failure fall
   back to `gap`.
5. Echo `bundle_hash` for harness pairing.
6. Print proposal JSON to stdout; exit 0.

### Invariants

- Stateless — no module-level state, no filesystem writes except stdout.
- No graph access — the bundle is the only source of truth.
- Output schema is strict.
- Single-shot — one Claude call per invocation (plus at most one retry).
- `gap` is a first-class outcome.
- Bare acknowledgements (empty or whitespace-only `reasoning`) are
  rejected at the worker boundary BEFORE stdout — the worker must
  produce reasoning or emit `gap`.

### Error handling

Identical exit-code mapping to [FT-126](FT-126):
- Bundle missing / malformed → exit 2.
- Claude API failure → exit 3.
- Schema validation failure after one retry → exit 4 with response.
- `AdrProposal.bundle_hash` ≠ input → exit 5.

### Boundaries

- **In scope.** Package layout (`workers/adr-author/`), Pydantic models,
  the prompt template, the Anthropic call, structured-output validation,
  `__main__` entry, ruff + pytest scaffolding, a unit test per proposal
  kind against mocked Claude.
- **Out of scope.** Persisting the proposed ADR or applying the
  acknowledgement (the harness handles `product adr new` + body update
  or `product feature acknowledge` writes only AFTER human acceptance
  per [ADR-075](ADR-075)). The paired judge ([FT-133](FT-133)
  adr-quality — one of four dedicated judges per [ADR-073](ADR-073)
  §"Why four judge roles, not fewer"). The default-acknowledge pass
  ([ADR-104](ADR-104)/[FT-104](FT-104)) runs BEFORE the worker is
  invoked.

## Out of scope

- Persistence — the worker prints JSON, never writes ADRs.
- Multi-gap batch proposals (one gap per call).
- Authoring features alongside the ADR (`spec-author` handles features).
- Amending existing ADRs (the amend flow runs through
  `product adr amend` with a separate human-driven prompt; this worker
  authors net-new ADRs only).
- Graduating ADR verdicts to L4 autonomy (see [ADR-075](ADR-075)
  §"Future graduation").
