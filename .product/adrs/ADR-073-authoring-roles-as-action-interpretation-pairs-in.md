---
id: ADR-073
title: Authoring roles as action-interpretation pairs in the readiness chain
status: proposed
features:
- FT-126
- FT-127
- FT-128
- FT-129
- FT-130
- FT-131
- FT-132
- FT-133
supersedes: []
superseded-by: []
domains:
- api
- data-model
scope: domain
---

**Status:** Proposed

## Context

[ADR-017](ADR-017) makes action-interpretation pairing **structural**: every
action session is paired with an interpretation session via a `DispatchGroup`;
the dispatch is not `complete` until both terminate and the interpretation
returns an `approved` verdict ([ADR-018](ADR-018)). SHACL refuses to mint a
`complete` group without the paired approved verdict. Slice 2 instantiated this
for one pair: implementer (action) → verifier (interpretation), under
[FT-021](FT-021). Slice 2.5/2.6 instantiated a second pair: verify-graph-author
(action, [ADR-030](ADR-030)) → graph executor (interpretation, slice 3).

The Definition-of-Ready chain ([FT-119](FT-119)) exposes the next four pairs
implicitly. Of FT-119's seven readiness dimensions, exactly one (`vgs_cover`)
is worker-resolvable today (via verify-graph-author). Every other gap is
`Stuck { reason }` because it requires human-authored content — a spec body, a
TC, an ADR — that no role in the catalog claims as its action. To turn the
planner from observe-only into a readiness orchestrator (per the brief that
commissions [ADR-076](ADR-076)) we need four new authoring roles. Once they
exist, the central claim of the framework forces their shape: each must be an
**action session paired with an interpretation**, or [ADR-017](ADR-017) becomes
optional and the framework's structural guarantee evaporates.

This ADR fixes the *role* layer of that shape. [ADR-074](ADR-074) fixes the
*verdict artifact* the interpretation produces; [ADR-075](ADR-075) fixes the
*acceptance autonomy* per artifact kind; [ADR-076](ADR-076) fixes the
*planner* that dispatches them in chain order.

## Decision

**Every authoring role in the readiness chain is an action session paired with
an interpretation, reusing [ADR-017](ADR-017)'s `DispatchGroup` machinery
verbatim. There is no parallel "authoring lifecycle" — the four new action
roles inherit dispatch-state transitions, SHACL pairing enforcement, and
bundle-hash provenance from FT-021. Each action role has its own dedicated
judge role with its own rubric.**

### The eight new roles (four authors, four judges)

| Action role | Interpretation role | Action artifact | Verdict | Source-of-truth (`dec:against`) |
|---|---|---|---|---|
| `tc-author` ([FT-126](FT-126)) | `tc-quality` ([FT-127](FT-127)) | `TcProposal` | `dec:QualityVerdict` | the feature_spec the TC must serve |
| `verify-graph-author` *(existing, [ADR-030](ADR-030))* | `vg-quality` ([FT-128](FT-128)) | `GraphProposal` | `dec:QualityVerdict` | the TC(s) + environment |
| `spec-author` ([FT-129](FT-129)) | `spec-quality` ([FT-132](FT-132)) | `SpecProposal` | `dec:QualityVerdict` | the originating request/brief |
| `adr-author` ([FT-130](FT-130)) | `adr-quality` ([FT-133](FT-133)) | `AdrProposal` or `Acknowledgement` | `dec:QualityVerdict` | the preflight gap + feature_spec |

The action role authors a candidate artifact in the same SDP-clean,
bundle-in / artifact-out shape established by [FT-048](FT-048): stateless,
single-shot, no graph access, strict Pydantic I/O, structured-output Claude
call, retry budget 1, `python -m <module> --stdin` entry point. The
match-vs-generate decision continues to live in the harness ([FT-046](FT-046)
pattern), not the worker.

The interpretation role consumes the candidate plus its source-of-truth (the
feature_spec for a TC, the TC+env for a VG, the request for a spec, the
preflight-gap+spec for an ADR) and emits a `dec:QualityVerdict` per
[ADR-074](ADR-074). The judge is structurally identical to the slice-2
verifier worker ([FT-023](FT-023)) — only the artifact class judged and the
rubric in the prompt differ.

### Why four judge roles, not fewer

Three reasons compound. Any one would justify per-action-role judges; all
three together close the door on consolidation:

1. **Distinct rubrics.** TC-quality scores against five criteria (clear,
   testable, non-redundant, faithful to spec, runner-wireable — see
   [FT-127](FT-127)). VG-quality scores against four (steps demonstrate the
   TC, `requiredOps ⊆ allowedOps` minimally, environment appropriate,
   evidence mapping sound — see [FT-128](FT-128)). Spec-quality scores
   against the [ADR-047](ADR-047) body-completeness schema plus
   request-faithfulness — different criteria, different controlled
   vocabulary. ADR-quality scores against the ADR H2 schema plus
   gap-closure soundness plus the "no bare acknowledgement" rule per
   [FT-130](FT-130). Collapsing two of these into a "prose-quality"
   judge with a rubric flag forfeits the rubric clarity that makes each
   judgment actionable.

2. **Distinct authority scopes.** Per [ADR-027](ADR-027), every role
   declares `dec:mayDecide` and `dec:mustEscalate`. Those scopes are
   per-rubric: tc-quality `mayDecide` includes test-style choices but
   `mustEscalate` includes the feature_spec's behaviour assertions;
   spec-quality `mayDecide` includes section-internal phrasing but
   `mustEscalate` includes the request's intent. Stuffing these into one
   judge means the worker either holds a merged `mustEscalate` list
   (over-cautious on every kind) or holds a kind-conditional list (no
   longer a flat declaration, harder to audit, drifts).

3. **Distinct feedback-flow targets.** Per [ADR-022](ADR-022)–
   [ADR-026](ADR-026), a judge emitting feedback routes it to a specific
   target role. A tc-quality `gap` is routed to spec-author (the TC
   wants a behaviour the spec doesn't describe). A spec-quality `gap` is
   routed back to the original requester (the brief under-specifies). A
   vg-quality `gap` is routed to whoever extends the step vocabulary. An
   adr-quality `gap` is routed to the architect / human reviewer. A
   single multiplexed judge cannot carry these distinct routing tables
   without becoming a router itself, which is the orchestrator's job.

Result: four distinct judge roles, each with its own catalog entry, its
own authority declaration, its own bundle shape, and its own measurable
agreement rate ([ADR-021](ADR-021)). The cost is four worker packages
instead of two; the benefit is rubric clarity, authority discipline,
and feedback-flow correctness from day one.

### What the planner observes

[ADR-076](ADR-076) defines the readiness orchestrator's classification table.
This ADR establishes the rule the planner consults: a readiness bit
(`tcs_ready`, `vgs_ready`, `spec_ready`, `adr_acks_ready`) flips to `true` if
and only if there exists a `dec:QualityVerdict` whose `dec:judges` is the
artifact, whose `dec:against` is the appropriate source-of-truth, and whose
`dec:verdict` is `approved` **and** whose enclosing `DispatchGroup` is in
status `complete` (i.e. the pair terminated, not just the interpretation).

The planner never reads provisional verdicts. Acceptance autonomy
([ADR-075](ADR-075)) decides *how* an approved verdict transitions to
"observed by the planner" — auto-flip versus human-accept — but the planner's
read predicate is uniform: `complete` group + `approved` verdict.

### Reuse, not invention

Per the brief (§3) every new role plugs into the existing seams:

- **DispatchGroup machinery** ([FT-021](FT-021), [ADR-017](ADR-017)) — minted
  per dispatched authoring round; state machine unchanged; SHACL pairing
  enforcement unchanged.
- **Worker package shape** ([FT-048](FT-048)) — each new role ships as a
  `workers/<role>/` package matching FT-048's contract byte-for-byte. The
  match-vs-generate split is harness-side ([FT-046](FT-046) pattern; the
  `tc-author` matcher is "feature already has ≥`min_tcs_per_feature` TCs"
  per [ADR-072](ADR-072)).
- **Worker resolver routing** ([FT-067](FT-067)) — each new role gets a
  `[[worker]]` entry in `manifest.toml`, a mirror in the `MANIFEST` constant,
  and `ACTIVE_ROLES_*` extension. TC-050's "no second resolution chain"
  invariant holds for all eight roles.
- **Authority declarations** ([ADR-027](ADR-027), [FT-030](FT-030)) — each new
  role declares `dec:mayDecide` and `dec:mustEscalate`, and a paired
  `dec:escalateVia` (class, target-role) per [ADR-023](ADR-023)/[ADR-026](ADR-026).
  The judge roles inherit the verifier's authority shape ([FT-019](FT-019)).
- **Verdict schema** ([ADR-018](ADR-018) shape, reused as the polymorphic
  basis for [ADR-074](ADR-074)).

The framework's central claim — that decisions and actions compose into
self-correcting chains — gains four pairs without inventing new substrate.

### Failure modes the rule handles

- **Authoring worker crashes.** Action session reaches `failed`; the group
  reaches `action-failed`. No judge is dispatched; the planner observes the
  readiness bit as still `false`. Inherited from [ADR-017](ADR-017) §"Failure
  modes."
- **Judge issues `rejected`.** The proposed artifact is provisional and
  marked superseded for the purposes of readiness; the group transitions to
  `interpretation-rejected`. The planner classifies the feature as `Stuck` on
  the relevant dimension *unless* a fresh authoring round produces a different
  candidate. Cycle-detection (PAT-002) catches author↔judge oscillation.
- **Judge issues `amendment-required`.** Group transitions to
  `awaiting-amendment`; a follow-up action dispatch consumes the
  `dec:amendmentGuidance` ([ADR-018](ADR-018)) as additional bundle context.
  Mirrors the verifier's amendment loop ([FT-021](FT-021)).
- **Authoring round produces no candidate (`Gap` outcome).** Inherited from
  [FT-048](FT-048): a worker may return `Gap { reason }` rather than a
  proposal. The harness routes the gap as feedback ([ADR-022](ADR-022)) and
  the planner stays `Stuck` with the gap reason cited.

### Why "structural" and not "policy"

For the same reason [ADR-017](ADR-017) is structural rather than configurable:
a framework that makes its central guarantee opt-in cannot certify the
guarantee. The four new authoring pairs MUST be authored-and-judged, not
authored-and-trusted, because authoring the criteria that the feature's own
code is later judged against is the highest-leverage trust boundary in the
system. The brief (§2.4) accepts the trust boundary in operator terms (trust
+ observe + course-correct via observability), but the *artifact-level*
guarantee — that an authored TC has a recorded judgment of fitness — is
structural. Acceptance autonomy ([ADR-075](ADR-075)) splits *when* that
judgment flips a bit; this ADR establishes that the judgment must exist.

## Rejected alternatives

- **One unified `quality-judge` worker with a rubric flag.** Rejected per
  "Why four judge roles" above: forfeits rubric clarity, fractures authority
  declarations, and forces a kind-conditional feedback-routing table that
  duplicates the orchestrator's job.
- **Two judges (one for code-shaped artifacts: TC+VG; one for prose: spec+ADR).**
  Considered as an intermediate position. Rejected for the same three reasons
  — TC and VG rubrics are not the same code-shaped rubric (one is
  test-quality, the other is graph-minimality + evidence-mapping), and spec
  and ADR rubrics are not the same prose-shaped rubric (one is
  request-faithfulness against an H2/H3 schema, the other is gap-closure
  soundness against a separate H2 schema with a no-bare-acknowledgement rule).
  Four is the minimum that preserves rubric independence.
- **Inline self-judgement.** Author and judge in one Claude call to halve
  cost. Rejected by [ADR-017](ADR-017) for code; same reason applies to TCs
  and graphs. The action-interpretation agreement metric ([ADR-021](ADR-021))
  is exactly the signal we need to measure for these new pairs once they are
  in use.
- **Skip judgement on authored artifacts; trust the planner to dispatch and
  let downstream sessions reject if the authored artifact is bad.** This is
  what [FT-119](FT-119) does today by being observe-only; the brief calls
  the gap explicitly. Skipping judgement at author time means the readiness
  bit is structurally `false` forever (no verdict exists), defeating the
  purpose of a readiness orchestrator. Rejected.
- **Reuse the slice-2 verifier worker ([FT-023](FT-023)) for all judges.** The
  verifier's rubric is "code satisfies feature_spec." TC quality is "TC is
  fit for an implementer to consume"; VG quality is "graph demonstrates the
  TC in the env, minimally"; spec quality is "spec faithfully serves the
  request"; ADR quality is "ADR closes the preflight gap soundly." Four
  different rubrics demand four different roles. The verifier and the quality
  judges share the *contract* (action-interpretation pair) but not the
  *rubric* or the *target class*.

## Consequences

**Positive:**

- The framework's central guarantee extends to the four new authoring roles
  without inventing new substrate. [ADR-017](ADR-017)'s structural pairing
  is reused four more times.
- Authority categories ([ADR-027](ADR-027)) gain eight new role entries with
  declared scopes (four authors + four judges); feedback-flow measurability
  ([ADR-022](ADR-022)–[ADR-026](ADR-026)) covers the new roles from day one.
- Action-interpretation agreement ([ADR-021](ADR-021)) becomes measurable for
  authored artifacts, not just for code. Slice C fitness functions
  ([ADR-014](ADR-014)) can compute disagreement rates per authoring role
  *and per judge role* independently.
- The planner ([ADR-076](ADR-076)) reads readiness via one uniform predicate
  (`complete` group + `approved` verdict + acceptance per [ADR-075](ADR-075)),
  not via four bespoke checks per artifact kind.

**Negative / accepted costs:**

- Every authoring round costs two LLM calls (author + judge), same trade as
  the implementer/verifier pair. For the slice where the framework is
  proving "authoring is paired the same as coding," the cost is correct.
- The role catalog ([FT-030](FT-030)) grows by eight entries (four authors +
  four judges), each with its own authority declaration. Per
  [ADR-027](ADR-027) §"Consequences," "every new role requires deliberate
  authoring of its authority"; the cost is acknowledged.
- Four worker packages to maintain instead of two. The two-judges-only
  alternative was considered for cost reasons and rejected on rubric grounds
  — see "Rejected alternatives."

**Enforcement:**

- [FT-021](FT-021)'s `DispatchGroup` SHACL refuses any `complete` group
  without an `approved` verdict reachable through the paired interpretation
  session. The new authoring pairs inherit that gate.
- A cross-cutting TC under [ADR-014](ADR-014) (authored in this ADR cluster's
  TC set — [TC-312](TC-312)) asserts every dispatched authoring session has
  a paired interpretation session reachable in the orchestration store.
- [ADR-072](ADR-072) (TC coverage floor) interacts: features authored by
  tc-author are subject to the `min_tcs_per_feature` floor and the baseline
  ramp. The tc-author's `kind: sufficient` outcome (no TCs needed) is a
  structural exit when the floor is already met, same shape as
  [FT-048](FT-048)'s `Match`.

## Status

Proposed. Foundational for [ADR-074](ADR-074), [ADR-075](ADR-075), and
[ADR-076](ADR-076); every feature spec in the readiness-orchestrator cluster
([FT-126](FT-126)–[FT-133](FT-133)) links to this ADR.
