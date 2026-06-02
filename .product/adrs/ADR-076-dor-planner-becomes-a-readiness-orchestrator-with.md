---
id: ADR-076
title: DoR planner becomes a readiness orchestrator with full upstream chain
status: proposed
features:
- FT-131
supersedes: []
superseded-by: []
domains:
- api
- observability
scope: domain
---

**Status:** Proposed

## Context

[FT-119](FT-119) lands the Definition-of-Ready planner: `dec drive def-ready
FT-XXX` walks seven readiness dimensions (spec completeness, preflight,
deps-done, TCs linked, TC quality, VG cover, VG accepted) and classifies the
feature into one of `{Done, DispatchVerifyGraphAuthor, Stuck { reason }}`. Of
the seven dimensions, exactly **one** (`vgs_cover`) is worker-resolvable via
the existing verify-graph-author worker ([FT-048](FT-048)). The other six are
`Stuck` because they require human-authored content (a spec body, an ADR
acknowledgement, a TC, a pending VG human-accept) that no role in the
catalog claims as its action.

FT-119's out-of-scope section flags this directly:

> **Auto-authoring missing TCs.** A future feature_spec may add a `tc-author`
> worker role + a `dec drive def-ready --author-tcs` opt-in. … Today the bar
> is human-authored TCs; the gate just refuses to call them ready.
>
> **Auto-authoring missing spec sections.** Same shape — a spec-author
> worker is plausible long-term but is not part of this slice's contract.

This ADR records the decision to invert that out-of-scope list. With the
four new authoring pairs landed ([ADR-073](ADR-073)) and their verdict class
defined ([ADR-074](ADR-074)) and their acceptance autonomy decided
([ADR-075](ADR-075)), the planner gains the capability to **drive the entire
upstream chain that an implementation session depends on**, not just the VG
arm.

The inversion is the point: where FT-119's planner is mostly `Stuck`, this
extension's planner is mostly worker-resolvable.

## Decision

**The Definition-of-Ready planner becomes a readiness orchestrator. Its
worker-resolvable arm expands from one row (`DispatchVerifyGraphAuthor`) to
the full upstream chain: spec → ADR → TC → VG, each authored and
quality-judged via the pairs established by [ADR-073](ADR-073). The
planner's observe-only invariant ([PAT-001](PAT-001)) is preserved: it reads
accepted verdicts and dispatches; it never judges.**

### What changes

The change is additive to FT-119, not a rewrite. FT-119's seven-dimension
table is preserved verbatim for the rows it covers, with two surface edits
and several new rows:

- **Rename `tcs_ok` → `tcs_ready`** per brief §2.3. Every quality flag in
  the chain now means "this artifact is ready for the later actions that
  consume it" — the flag is the `approved` QualityVerdict observed (after
  acceptance per [ADR-075](ADR-075)) for the artifact under judgement.
- **Rename and reshape `vgs_accepted` → `vgs_ready`**, parallel to
  `tcs_ready`. (The existing `vgs_accepted` semantic is preserved as the
  underlying read; the rename surfaces the quality dimension at the planner
  level.)
- **New dimensions:** `spec_ready`, `adr_acks_ready` (the latter is the
  quality counterpart to FT-119's `preflight` row when adr-author has been
  dispatched to fill the gap).

### New `Action` variants

Added to `core::drive::action::Action` (the variant set is exhaustively
matched by every planner, per [PAT-001](PAT-001)):

- `DispatchTcAuthor { feature_id, target_count }` — dispatch tc-author when
  `tcs_linked` is below `min_tcs_per_feature` ([ADR-072](ADR-072)) AND no
  pending TC proposal exists for the feature.
- `DispatchTcQuality { feature_id, tc_proposal_iri }` — dispatch tc-quality
  on a pending TC proposal that has no QualityVerdict yet.
- `DispatchVgQuality { feature_id, graph_proposal_iri }` — dispatch
  vg-quality on a pending VG proposal that has no QualityVerdict yet (the
  vg-quality interpretation half of the existing vg-author pair).
- `DispatchSpecAuthor { feature_id }` — dispatch spec-author when
  `spec_complete` is `false` AND no pending spec proposal exists.
- `DispatchAdrAuthor { feature_id, preflight_gap }` — dispatch adr-author
  on an unacknowledged preflight gap when no pending ADR/Acknowledgement
  proposal exists.

The existing `DispatchVerifyGraphAuthor` variant is unchanged. The five new
variants slot into the worker-resolvable arm of the classification table.

### New inspector methods

Added to the `GraphInspector` trait (per [PAT-001](PAT-001)). Production
impls read from the orchestration store + product graph; stubs ship with
the unit tests:

- `tc_quality_verdicts(feature_id) -> Vec<QualityVerdictRecord>` — the set
  of QualityVerdicts whose `dec:judges` is a TC linked to the feature,
  filtered by acceptance ([ADR-075](ADR-075)) — auto-accepted verdicts are
  included; pending-review verdicts are not.
- `vg_quality_verdicts(feature_id) -> Vec<QualityVerdictRecord>` — same
  for VGs.
- `spec_quality_verdict(feature_id) -> Option<QualityVerdictRecord>` —
  the single most-recent spec-quality verdict for the feature; `None` if
  none exists.
- `adr_acks_quality_verdicts(feature_id) -> Vec<QualityVerdictRecord>` —
  the set of verdicts judging ADR proposals against the feature's
  preflight gaps.
- `pending_proposals(feature_id) -> ProposalSet` — the set of in-flight
  proposals (TC, VG, spec, ADR) that have not yet been judged. Used to
  avoid re-dispatching authors over their own in-flight work.

The existing `tcs_with_runner_state` (FT-119) is renamed
`tcs_ready` and now consults `tc_quality_verdicts` rather than reading the
TC's `runner` / `runner-args` frontmatter directly. The structural
"runner is wired" check moves into the tc-quality judge's rubric where it
belongs.

### Expanded classification table (precedence-ordered, first match wins)

```
spec     adr_acks    preflight   deps      tcs_linked    tcs_ready     vgs_cover     vgs_ready    │  Action
─────────────────────────────────────────────────────────────────────────────────────────────────┼─────────────────────────────────────────
ready    ready       clean       done      true          true          true          true        │  Done
*        *           *           false     *             *             *             *           │  Stuck "blocked: <FT-Y status>"
false    *           *           true      *             *             *             *           │  DispatchSpecAuthor { feature_id }                    (Slice B)
pending  *           *           true      *             *             *             *           │  Stuck "spec pending human-accept: <proposal-iri>"   ([ADR-075](ADR-075))
ready    false       warnings    true      *             *             *             *           │  DispatchAdrAuthor { feature_id, preflight_gap }      (Slice B)
ready    pending     warnings    true      *             *             *             *           │  Stuck "adr pending human-accept: <proposal-iri>"    ([ADR-075](ADR-075))
ready    ready       warnings    true      *             *             *             *           │  Stuck "preflight: <gap list>"                       (no adr-author resolution path)
ready    ready       clean       true      false         *             *             *           │  DispatchTcAuthor { feature_id, target_count }
ready    ready       clean       true      pending       *             *             *           │  DispatchTcQuality { feature_id, tc_proposal_iri }
ready    ready       clean       true      true          false         *             *           │  Stuck "tc rejected: <tc-id>"                        (judge said no; cycle backstop)
ready    ready       clean       true      true          true          false         *           │  DispatchVerifyGraphAuthor { feature_id, env_id }    (FT-119, preserved)
ready    ready       clean       true      true          true          pending       *           │  DispatchVgQuality { feature_id, graph_proposal_iri }
ready    ready       clean       true      true          true          true          false       │  Stuck "vg rejected: <vg-id>"                        (judge said no; cycle backstop)
```

Reading order: top → bottom; first match wins. The shape preserves FT-119's
ordering invariant (preflight > deps > spec > tcs_linked > tcs_ready >
vgs_cover > vgs_ready) and inserts the new dispatch arms between the Stuck
rows they resolve.

Per-row notes:

- `pending` is a three-valued state for each dimension: `ready` (verdict
  approved + accepted), `pending` (verdict in `pending_review` per
  [ADR-075](ADR-075) — human-accept artifact kinds only), `false`
  (verdict missing or rejected). Auto-accepted kinds (TC, VG) collapse
  to two values.
- `Stuck "preflight: <gap list>"` is reached only when adr-author has
  dispatched and returned an `Acknowledgement` proposal that the planner
  treats as exhausting the worker arm for that gap. Operators who want
  human-authored ADRs override via `--no-author` (below).

### `--no-author` escape hatch

Per brief §2.6 + §4C: **authoring is on-by-default** under `def-ready`. No
opt-in flag required to dispatch authors. The escape hatch is the
opposite — `dec drive def-ready FT-XXX --no-author` restores FT-119's
observe-only behaviour: every author-dispatch row becomes a `Stuck` row
with the corresponding "needs human authoring" reason. Useful for power
users who want the FT-119 gate semantics without the orchestrator
behaviour.

`--no-author` also propagates to `--all`. The `core::drive::sweep`
machinery passes it through to each per-feature drive (PAT-003 unchanged).

The verb stays `def-ready`, not `ready`. FT-119 rejected dropping the
`def-` prefix as a CLI-naming concern; that concern is unchanged here.

### Preserving FT-119's invariants

- **Pure classification.** `FeatureReadyPlanner::classify` remains a pure
  function of inspector observations ([PAT-001](PAT-001)). New `Action`
  variants extend the enum; new inspector methods feed the new dimensions.
  No I/O, no time, no global state.
- **Observe-only.** The planner reads accepted QualityVerdicts; it never
  judges, accepts, or rejects. Acceptance autonomy ([ADR-075](ADR-075)) is
  a harness concern downstream of the planner's read predicate.
- **Cycle detection.** [PAT-002](PAT-002)'s `state_hash_for_feature`
  extends to hash the new dimensions and pending-proposal counts. An
  author↔judge oscillation (author writes → judge rejects → re-author →
  judge rejects, ad infinitum) yields a period-N cycle and `Stuck` with a
  graph-theoretic reason before `max_iter`.
- **Sweep contract.** [PAT-003](PAT-003)'s
  `core::drive::sweep::drive_one_feature_with_timeout` is unchanged in
  shape. `--all` continues to enumerate features and bound per-feature
  runtime; `--filter`, `--format`, `--max-iter`, `--per-feature-timeout`,
  `--bench` are inherited.
- **No write authority for the planner.** The planner does not write to
  the product graph or the orchestration store; only the dispatched author
  / judge workers (and harness-level acceptance flips) do.

### Relationship to FT-119

FT-119 is **not invalidated.** Per the brief's framing (§1), this ADR
records the *slice direction* FT-119 explicitly deferred. FT-119 remains
correct for its scope: a Definition-of-Ready gate with one resolvable
arm. The readiness-orchestrator feature ([FT-131](FT-131)) is the
successor that lands the full chain.

Concretely:

- FT-119's classification table is the strict subset of this table where
  `spec_ready=*`, `adr_acks_ready=*`, and only the `vgs_cover=false`
  worker-resolvable row exists.
- FT-119's TC set (TC-253..TC-258) continues to validate the rows it
  covers. The successor feature ([FT-131](FT-131)) adds TCs for the new
  rows.
- The CLI verb is unchanged. `dec drive def-ready FT-XXX` is the same
  entry point with strictly more behaviour.

The supersedes-by link is not set on FT-119 because the structural
invariant ("observe-only planner") is reused, not replaced. The brief
draws the parallel to ADR-028 succeeding ADR-020's direction without
invalidating it.

## Rejected alternatives

- **Hard-rewrite FT-119 instead of extending it.** Rejected per the brief:
  "FT-119 remains correct for the slice it governs — do not rewrite it
  past recognition." The extension lands as a successor feature_spec, not
  a re-author. The classification table grows by additive rows.
- **Drop the `def-` prefix on the verb.** Rejected by FT-119 and unchanged
  here. The CLI naming concern (overloading "ready" with future meanings)
  still applies.
- **Authoring opt-in (a flag to enable each author kind).** Rejected per
  §2.6: authoring is on-by-default. The opposite escape hatch
  (`--no-author`) is provided for operators who want the FT-119 gate
  semantics back.
- **Per-author flags (`--author-tcs`, `--author-specs`).** Rejected: the
  brief's authoring-by-default decision makes the chain coherent; per-arm
  flags would let operators leave the chain partially driven, producing
  hard-to-reproduce planner states. `--no-author` is the single inverse
  and `--filter` is the single per-feature targeting knob.
- **Move acceptance autonomy decisions into the planner.** Rejected per
  [ADR-075](ADR-075): the planner reads a uniform predicate (`complete`
  group + `approved` verdict + accepted) and is agnostic to per-kind
  autonomy. The split between auto-accept and human-accept lives in the
  harness.
- **Author DispatchSpecAuthor / DispatchAdrAuthor in Slice A.** Rejected
  per brief §5 build-order: prose authoring has higher trust boundary and
  no immediate VGA-style precedent. Slice A delivers TC/VG arms; Slice B
  delivers prose arms.
- **Use the existing verifier role ([FT-023](FT-023)) as the quality
  judge for authored artifacts.** Rejected per [ADR-073](ADR-073): different
  rubric, different target class, different role-catalog entry. The
  contract shape is shared; the rubric is not.

## Consequences

**Positive:**

- The planner becomes mostly worker-resolvable. The originally-requested
  capability ("verify the TCs, write them if missing; verify the graph,
  it's already wired") lands in Slice A.
- The framework's central guarantee ([ADR-017](ADR-017)) extends to four
  more authoring pairs without inventing parallel substrate.
- `deps_done` stays human-Stuck — full autonomy is never the goal. The
  planner being mostly-resolvable does not mean the system runs without
  humans; it means humans aren't blocking on artifact authoring that the
  framework can drive.
- Slice A delivers the TC/VG arms; Slice B can land or hold without
  blocking Slice A's value. The slicing maps to the build order in the
  brief (§5).
- FT-119's TC set continues to validate its rows. Tests are additive, not
  rewrites.

**Negative / accepted costs:**

- The classification table grows from 8 rows to 13. The "first match
  wins" precedence is what keeps the table comprehensible; the
  pure-classification TC ([TC-254](TC-254) shape) is the comprehensive
  backstop.
- Per-author dispatch rounds add LLM cost. For features whose chain is
  mostly authored, the cost is a one-time investment; subsequent
  `def-ready` runs are cheap (the readiness bits are already flipped).
- Acceptance autonomy per [ADR-075](ADR-075) creates pending-review
  queues for spec/ADR proposals; operators carry latency on those kinds.
  The CLI surfacing of those queues is owned by [FT-131](FT-131).

**Enforcement:**

- A pure-classification TC over the expanded table, mirroring
  [TC-254](TC-254)'s shape against a `StubInspector`. Every cell maps to
  the expected `Action`.
- A Stuck-reason-identity TC: every `Stuck` reason cites the offending
  artifact id verbatim through the driver, mirroring [TC-255](TC-255).
- An integration TC per new dispatch arm: planner dispatches the author
  when the artifact is missing, dispatches the quality judge, and flips
  to the next state once the verdict is accepted; returns `Done` only
  when the whole chain is authored-and-judged-approved.
- A cycle-detection TC for an author↔judge oscillation, yielding
  `Stuck` with a period-N reason before `max_iter`.
- A `--no-author` regression TC: `dec drive def-ready FT-XXX --no-author`
  reproduces FT-119's classifications byte-for-byte on a feature whose
  chain has gaps.

## Status

Proposed. Linked to [ADR-073](ADR-073) (paired authoring),
[ADR-074](ADR-074) (verdict class), [ADR-075](ADR-075) (acceptance
autonomy), [FT-119](FT-119) (the slice this succeeds), and
[FT-126](FT-126)–[FT-131](FT-131) (the implementing features).
