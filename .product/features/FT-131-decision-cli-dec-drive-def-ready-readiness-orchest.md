---
id: FT-131
title: 'decision-cli: dec drive def-ready readiness-orchestrator extension — full upstream chain'
phase: 4
status: complete
depends-on:
- FT-119
- FT-126
- FT-127
- FT-128
- FT-021
- FT-067
adrs:
- ADR-076
- ADR-073
- ADR-074
- ADR-075
- ADR-072
tests:
- TC-306
- TC-307
- TC-308
- TC-309
- TC-310
- TC-311
domains: []
domains-acknowledged:
  ADR-071: ADR-071 governs in-process worker tool calls. FT-131 is a pure-classification planner extension (per PAT-001) with no tool calls of its own; it dispatches out-of-process author/judge workers whose tool boundaries are spec'd in their own feature documents. Workspace-containment does not apply.
  ADR-070: ADR-070 governs role-scoped tool surfaces declared in the role catalog. FT-131 is the planner/orchestrator that dispatches workers via the FT-067 shared resolver; the tool surfaces of the dispatched roles are declared by the worker feature_specs (FT-126..FT-130). FT-131 itself does not introduce or modify role-catalog entries.
---

## Description

Successor feature to [FT-119](FT-119): the Definition-of-Ready planner
becomes a **readiness orchestrator**. Per [ADR-076](ADR-076), the
worker-resolvable arm expands from one row (the existing
`DispatchVerifyGraphAuthor` over a missing VG) to the **full upstream
chain** that an implementation session depends on: feature_spec → ADR
acknowledgements → TCs → VerificationGraph + quality on each. Authoring is
**on-by-default** under `dec drive def-ready`; the escape hatch
`--no-author` restores FT-119's observe-only behaviour.

FT-119 is preserved, not invalidated. Its classification table is the
strict subset of this feature's table where `spec_ready=*`,
`adr_acks_ready=*`, and only the `vgs_cover=false` worker-resolvable row
exists. The CLI verb (`dec drive def-ready FT-XXX`) is unchanged; the
behaviour expands.

The new dispatch arms slot into Slice A and Slice B per the brief (§5):

- **Slice A (this feature):** `DispatchTcAuthor` ([FT-126](FT-126)),
  `DispatchTcQuality` ([FT-127](FT-127)), `DispatchVgQuality`
  ([FT-128](FT-128)). Plus the existing
  `DispatchVerifyGraphAuthor` ([FT-048](FT-048)) arm.
- **Slice B (gated behind this feature):** `DispatchSpecAuthor`
  ([FT-129](FT-129)), `DispatchAdrAuthor` ([FT-130](FT-130)).

The Slice B arms are spec'd in the classification table here so the
table is the single source of truth, but the dispatch rows are reachable
only once [FT-129](FT-129) / [FT-130](FT-130) land and the planner is
wired to invoke them. Until then the rows fall through to `Stuck` with
the "needs human authoring" reasons FT-119 emits today.

## Functional Specification

### Inputs

CLI surface (additive to [FT-119](FT-119)):

- `dec drive def-ready FT-XXX [--max-iter N] [--bench BNCH-NNN] [--no-author]`
  — single-feature drive. Defaults: `--max-iter 6` (raised from FT-119's
  4 to accommodate longer author/judge chains; tunable via
  [ADR-068](ADR-068) `[driver] max_iter`). `--no-author` disables every
  `Dispatch*Author` and `Dispatch*Quality` arm, falling back to
  FT-119's classification.
- `dec drive def-ready --all [--max-iter N] [--per-feature-timeout SECS]
  [--filter FT-A,FT-B,...] [--format text|tsv|json] [--bench BNCH-NNN]
  [--no-author]` — multi-feature sweep. Inherits [FT-111](FT-111)'s
  sweep contract verbatim through [PAT-003](PAT-003) (already lifted to
  `core::drive::sweep` per FT-119).

Internal substrate inputs (unchanged in shape from FT-119, expanded in
content):

- `PlanContext` — unchanged.
- `GraphInspector` trait gains the new methods listed in §"State" below.
- Reads from the product graph + the orchestration store for
  QualityVerdict artifacts ([ADR-074](ADR-074)) and pending-proposal
  artifacts.

### Outputs

Per single-feature drive: same `DriveOutcome::Reached { iterations,
history }` shape as FT-119. The history captures every dispatched
author / judge round.

Per multi-feature sweep: same `SweepRow` / `SweepTally` shape as
[FT-111](FT-111). The goal column reads `def-ready`.

Stuck reasons are formatted per dimension:

| Stuck cause | Reason format |
|---|---|
| Dep not done | `blocked: FT-Y status=in-progress` |
| Spec pending human-accept (Slice B) | `spec pending human-accept: <proposal-iri>` |
| ADR pending human-accept (Slice B) | `adr pending human-accept: <proposal-iri>` |
| Preflight ADR with no author path | `preflight: <ADR-NNN list>` (FT-119 verbatim) |
| TC verdict rejected after author round | `tc rejected: TC-XXX (verdict-iri)` |
| VG verdict rejected after author round | `vg rejected: VG-XXX (verdict-iri)` |
| Author↔judge cycle | `cycle: <period> rounds on <dim>` |

`--no-author`-active runs prefix every Stuck row with
`[no-author] ` so the operator can distinguish "FT-119 behaviour" from
"FT-131 behaviour" in mixed sweeps.

New code organisation (additive to FT-119; the brief mandates no rewrite):

- `crates/decision-cli/src/features/ft_131_readiness_orchestrator/`
  - `cli.rs` — `--no-author` flag plumbing (additive to FT-119's
    argparse). The single-feature and `--all` paths delegate to the
    same sweep skeleton as FT-119.
  - `planner.rs` — `FeatureReadyPlanner<I: GraphInspector>` rewrite of
    FT-119's planner module. Pure `classify(feature_id, bench_id,
    no_author: bool) -> Result<Action, PlanError>` with the expanded
    table.
  - `inspect.rs` — production impl of the new `GraphInspector` methods
    over `PlanContext`; stubs for tests.
  - `dispatch.rs` — per-`Action` dispatch handlers that drive the
    paired author/judge dispatch through [FT-021](FT-021)'s
    `DispatchGroup` machinery.
  - `tests.rs` — unit + integration tests over the expanded table.
- `crates/decision-cli/src/core/drive/action.rs` — extend the `Action`
  enum with `DispatchTcAuthor`, `DispatchTcQuality`, `DispatchVgQuality`,
  `DispatchSpecAuthor`, `DispatchAdrAuthor` (Slice B variants gated
  behind a feature flag `slice_b` at the crate level — they exist in
  the enum unconditionally but the production dispatch handlers return
  `not-yet-implemented` until [FT-129](FT-129)/[FT-130](FT-130) land).
- `crates/decision-cli/src/core/drive/registry.rs` — re-register
  `FeatureReadyPlanner` for `(Feature, DefReady)` pointing at the new
  module. FT-119's planner becomes a dead path; the rewrite shares its
  TC set per the table-row mapping in [ADR-076](ADR-076).

### State

- No new on-disk schema. The `dec:QualityVerdict` class
  ([ADR-074](ADR-074)) lands under a separate feature spec authored
  alongside the worker package shipping the SHACL (the brief notes the
  shape lives in `core/ontology/quality.ttl`); this feature consumes
  the class via SPARQL queries.
- New `GraphInspector` methods (production impl + stub):
  - `tc_quality_verdicts(feature_id) -> Vec<QualityVerdictRecord>`.
  - `vg_quality_verdicts(feature_id) -> Vec<QualityVerdictRecord>`.
  - `spec_quality_verdict(feature_id) -> Option<QualityVerdictRecord>`.
  - `adr_acks_quality_verdicts(feature_id) -> Vec<QualityVerdictRecord>`.
  - `pending_proposals(feature_id) -> ProposalSet` — TC/VG/spec/ADR
    proposals in-flight without a paired verdict, used by the planner
    to avoid re-dispatching authors over their own work.
- The existing `tcs_with_runner_state` renames to `tcs_ready` and now
  consults `tc_quality_verdicts` rather than reading TC frontmatter
  directly. The structural "runner is wired" check moves into the
  tc-quality judge's rubric per [FT-127](FT-127).
- Writes (transitive, via dispatched actions only): author workers
  produce proposals; judge workers produce `dec:QualityVerdict`
  artifacts; the harness materialises authored artifacts after the
  paired judge approves and acceptance per [ADR-075](ADR-075) flips
  the readiness bit. The planner itself never writes.

### Behaviour

1. Parse `dec drive def-ready <FT-XXX | --all> [--no-author] ...`.
2. Look up `planner_for(Feature, DefReady)` — points at the FT-131
   planner.
3. For single-feature mode: invoke `drive::run` with `max_iter` and the
   resolved bench. The loop is FT-119's: `plan → dispatch → re-plan`.
   New `Action` variants reach new dispatch handlers.
4. For `--all` mode: identical sweep contract to FT-119; the additional
   work happens inside per-feature `drive::run`.
5. Classification: read all eight dimensions
   (`spec_ready`, `adr_acks_ready`, `preflight`, `deps_done`,
   `tcs_linked`, `tcs_ready`, `vgs_cover`, `vgs_ready`) plus
   `pending_proposals`. Walk the precedence table from
   [ADR-076](ADR-076) §"Expanded classification table"; first match
   wins.
6. Under `--no-author`: every `Dispatch*Author` and `Dispatch*Quality`
   row collapses to `Stuck` with FT-119's reason format. The
   classification is otherwise identical.
7. Dispatch handlers per `Action`:
   - **`DispatchTcAuthor`** — mint `DispatchGroup`, dispatch
     [FT-126](FT-126), await action session terminal. On success,
     transition to `awaiting-interpretation`; the next planner
     iteration matches `tcs_linked: pending` and dispatches
     `DispatchTcQuality`.
   - **`DispatchTcQuality`** — dispatch [FT-127](FT-127) with the
     pending TC proposal in the bundle. On verdict approved, harness
     persists the proposed TCs (`product test new` + `product test
     runner`) and acceptance per [ADR-075](ADR-075) auto-flips the
     `tcs_ready` bit. On verdict rejected → `Stuck "tc rejected"`. On
     amendment-required → re-dispatch FT-126 with guidance.
   - **`DispatchVerifyGraphAuthor`** — unchanged from FT-119 +
     [FT-049](FT-049). After action terminal, dispatch
     `DispatchVgQuality`.
   - **`DispatchVgQuality`** — dispatch [FT-128](FT-128). Same shape
     as `DispatchTcQuality`.
   - **`DispatchSpecAuthor`** / **`DispatchAdrAuthor`** — gated to
     Slice B. The dispatch handler exists but returns
     `not-yet-implemented` until [FT-129](FT-129)/[FT-130](FT-130)
     land. The corresponding rows in the table currently fall through
     to `Stuck` for ADR/spec gaps until then.

The reuse of FT-119's `drive::run` is deliberate: this feature does not
re-author the harness's iteration loop, only the per-`Action`
classifications and the new dispatch handlers.

### Invariants

- **Pure classification.** `FeatureReadyPlanner::classify` remains a
  pure function of inspector observations
  ([PAT-001](PAT-001)) plus the boolean `no_author`. Same testability
  contract as FT-119.
- **Observe-only.** The planner reads accepted QualityVerdicts; it never
  judges, accepts, or rejects.
- **A `Done` outcome is a structural guarantee.** Inherited verbatim
  from FT-119: a feature reaching `Done` will pass `product preflight`,
  `product graph check`, FT-045 coverage queries for every TC, AND will
  have an approved-and-accepted QualityVerdict for every authored
  artifact in its chain.
- **Cycle detection.** [PAT-002](PAT-002)'s `state_hash_for_feature`
  extends to hash the five new dimensions + the pending-proposal
  counts. Author↔judge oscillation yields a period-N cycle and `Stuck`
  with a graph-theoretic reason before `max_iter`.
- **Sweep contract.** [PAT-003](PAT-003)'s
  `core::drive::sweep::drive_one_feature_with_timeout` is unchanged in
  shape; `--all` continues to enumerate features, deterministic
  ordering, bounded total runtime.
- **No write authority for the planner.** The planner does not write
  to the product graph or the orchestration store. Author / judge
  workers and harness acceptance flips do all writes.
- **FT-119 byte-for-byte parity under `--no-author`.** A `--no-author`
  run on any feature that FT-119 classified produces the same `Action`
  string and the same `Stuck` reason text. The TC-119 fixture set is
  reused as a regression backstop.

### Error handling

Inherited from FT-119:

- `--all` with empty graph / unknown `--filter` IDs → fail fast, exit
  non-zero, surface offending IDs.
- Inspector returns `Err(InspectError::StoreUnreadable)` →
  `SweepOutcome::Error { detail }` in `--all`; propagates in single.
- Author / judge dispatch returns `Err` (worker crashed, validator
  refused) → driver does NOT swallow it; history captures the failing
  action; `--all` records `Error`.
- `Stuck` reason carrying an artifact id (`tc rejected: TC-XXX`) MUST
  cite the offending artifact verbatim, mirroring FT-119's TC-255
  contract.
- Inconsistent state: a TC proposal exists with no paired verdict AND
  no in-flight judge dispatch — the planner classifies as
  `DispatchTcQuality` (the recovery path), not `Stuck`. Same shape
  inherited from FT-119's VG superseded-but-no-successor handling.

### Boundaries

- **In scope.** New `Action` variants (all five), new inspector
  methods, the expanded classification table, the `--no-author` flag,
  per-`Action` dispatch handlers (with Slice B handlers stubbed as
  `not-yet-implemented`), the FT-119 regression fixture reuse, the
  PAT-002 hash extension, the pending-proposal taxonomy.
- **Out of scope.** The author/judge workers themselves (live in
  [FT-126](FT-126)–[FT-130](FT-130)). The `dec:QualityVerdict` SHACL
  shape (a separate feature spec authored alongside Slice A workers,
  per [ADR-074](ADR-074)). The fitness function watching auto-accept
  agreement (separate TC under [ADR-014](ADR-014)). The
  human-acceptance CLI for spec/ADR proposals (`dec drive accept` is
  a Slice B feature; this feature surfaces the pending-review state in
  Stuck reasons but does not add the acceptance verb).

## Out of scope

- Auto-acceptance of spec / ADR proposals (governed by
  [ADR-075](ADR-075) — human-accept).
- Per-author flags (`--author-tcs`, `--author-specs`). The brief (§2.6)
  fixes authoring-by-default; the single inverse is `--no-author` and
  the single per-feature targeting knob is `--filter`.
- Parallelising per-feature drives (inherited from FT-111 /
  [PAT-003](PAT-003)).
- A `--watch` mode (operators use [FT-113](FT-113)
  `dec drive show --watch` per feature).
- Migrating FT-119's TC set IDs (TC-253..258); they continue to
  validate the rows they cover. The new TCs for this feature are
  additive.
- Graduating any author/judge pair to L5 autonomy (see
  [ADR-075](ADR-075) §"Future graduation").
