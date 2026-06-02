---
id: FT-119
title: 'decision-cli: dec drive def-ready — Definition-of-Ready planner with per-feature and --all sweep'
phase: 4
status: complete
depends-on:
- FT-110
- FT-111
- FT-052
- FT-045
adrs:
- ADR-030
- ADR-031
- ADR-011
tests:
- TC-253
- TC-254
- TC-255
- TC-256
- TC-257
- TC-258
domains:
- api
domains-acknowledged:
  api: Adds `Goal::DefReady` to the existing FT-110 planner registry and reuses FT-111's sweep contract verbatim through a generic lift into `core::drive::sweep`. The CLI surface is a new verb on an existing namespace; no new public artifact-graph contract, SHACL shape, or worker schema. Reuses PAT-001 (Inspector/Planner) and PAT-003 (sweep) which already cite the relevant api-domain ADRs (ADR-011 CLI shape, ADR-029 CLI/MCP pairing).
patterns:
- PAT-001
- PAT-002
- PAT-003
---

## Description

Today there is no machine-checkable answer to **"is FT-XXX ready for the
implementer?"**. Operators eyeball a feature_spec, glance at the linked TCs,
hope a VerificationGraph exists, and dispatch `dec implement FT-XXX`. When the
spec is thin, the TCs are stubs, or no VG covers the TCs, the implementer
session burns budget producing code against a broken bundle — the verifier
then complains, defects route back, and the loop chews iterations before the
operator realises the *upstream* artifacts were never ready.

This feature introduces a **Definition of Ready (DoR)** as a first-class goal
in the `dec drive` planner family ([FT-110](FT-110)). A feature is *ready*
when, and only when, the implementer's context bundle is mechanically
sufficient:

1. The feature_spec body passes the FT-055/ADR-047 H2/H3 completeness check.
2. `product preflight FT-XXX` returns `status: clean` — every cross-cutting
   ADR is acknowledged, every domain gap is either linked or acknowledged
   with reasoning, and dependencies are available.
3. Every `depends-on` feature is `status: complete`.
4. Every TC the feature links exists, has a complete body, AND has a
   `runner` + `runner-args` pair wired (TC frontmatter agrees with what the
   test points at — the most common headless-run failure per CLAUDE.md).
5. A non-superseded `dec:VerificationGraph` exists that covers every TC
   listed in `feature.tests` via `dec:providesEvidenceFor`
   ([ADR-030](ADR-030), [FT-045](FT-045) coverage primitive).
6. Every covering VG has been accepted out of `pending_review`
   ([ADR-030](ADR-030) Level-3 autonomy — a proposal is not yet a graph).

When DoR fails because a VG is missing or incomplete, the planner dispatches
the existing **verify-graph-author** worker ([FT-049](FT-049), [FT-107](FT-107))
the same way `FeatureShipPlanner` does today. Everything else — missing
preflight ADR links, an unwritten TC body, a depends-on still in-progress —
is `Stuck { reason }` because it requires human-authored content, not worker
output. The Stuck reason cites the exact gap so the operator can act.

The slice ships:

- `Goal::DefReady` added to the `core::drive::goal::Goal` enum.
- `FeatureReadyPlanner` (new) in
  `features/drive/planners/feature_ready.rs`, registered for
  `(ArtifactKind::Feature, Goal::DefReady)` via the existing
  `planner_for(...)` registry.
- New inspector methods on the existing `GraphInspector` trait
  (PAT-001) for the DoR-specific dimensions: `feature_spec_completeness`,
  `preflight_status_for_feature`, `tcs_with_runner_state`,
  `covering_graphs_for_feature_tcs`, `dependency_status_map`. Production
  impls reuse FT-052's preflight reader, FT-045's coverage primitive, and
  the existing TC/VG SPARQL helpers; stubs ship for tests.
- `dec drive def-ready FT-XXX` single-feature command (mirrors
  `dec drive ship FT-XXX`).
- `dec drive def-ready --all` multi-feature sweep, factored through the
  same PAT-003 sweep skeleton as [FT-111](FT-111) — same flags
  (`--max-iter`, `--per-feature-timeout`, `--filter`, `--format`,
  `--bench`), same row/tally types reused via a generic over goal.

This is the gate operators have been hand-rolling: a structural,
non-negotiable contract that says "everything the implementer's bundle
will inject for FT-XXX is present and verified, and the worker will not
be set up to fail."

## Functional Specification

### Inputs

CLI surface (additions to the existing `dec drive` subcommand from FT-110 +
FT-111):

- `dec drive def-ready FT-XXX [--max-iter N] [--bench BNCH-NNN]`
  — single-feature drive. Defaults: `--max-iter 4` (a DoR drive only
  dispatches the VGA — 4 iterations is plenty for one or two
  re-authorings), `--bench` falls back to the workdir's default.
- `dec drive def-ready --all [--max-iter N] [--per-feature-timeout SECS]
  [--filter FT-A,FT-B,...] [--format text|tsv|json] [--bench BNCH-NNN]`
  — multi-feature sweep, semantics identical to FT-111's
  `dec drive ship --all` except (a) the goal is `def-ready` and
  (b) `--retire-failing-graphs` is intentionally omitted: DoR cares about
  *whether covering graphs exist*, not about retiring graphs that failed
  to run. Operators wanting to re-author can pass `--filter` and
  per-feature redrive.
- `--all` and `FT-XXX` are mutually exclusive at the argument parser
  level (same constraint as FT-111).

Internal substrate inputs:

- `PlanContext` carries (orchestration store handle, working tree root,
  product graph root, default bench) — unchanged from FT-110.
- The new `GraphInspector` methods read from:
  - the product-cli graph (via the FT-052 preflight reader and the
    feature-spec / TC frontmatter projections);
  - the orchestration store (for `dec:VerificationGraph` and
    `dec:pending_review` state); and
  - `.dec/verify/graph/*.ttl` for VG presence on disk.

### Outputs

Per single-feature drive (`dec drive def-ready FT-XXX`):

- On `Action::Done` — exit 0, `DriveOutcome::Reached { iterations,
  history }` rendered as the standard FT-110 text/json shape. The history
  captures every dispatched VGA round.
- On `Action::Stuck { reason }` — exit non-zero, reason string preserved
  verbatim and prefixed with `FT-XXX def-ready:`.
- On `Err::MaxIterations` — exit non-zero with the iteration count +
  history (same shape FT-110 already emits).

Per multi-feature sweep (`dec drive def-ready --all`):

- Stdout: per-row outcome (FT-id, outcome enum, elapsed_ms, reason) +
  aggregate tally — same `SweepRow` / `SweepTally` types FT-111 already
  defined, reused via generics over `Goal`. Exit 0 if every feature ended
  `Done`, otherwise exit 1.
- `--format json` emits the structured row+tally JSON shape; `--format
  tsv` matches FT-111's tsv shape verbatim with the goal column populated
  as `def-ready`.

New code organisation:

- `crates/decision-cli/src/features/ft_119_drive_def_ready/`
  - `cli.rs` — argparse plumbing for both single and `--all` modes,
    delegating to FT-111's sweep skeleton when `--all` is set.
  - `planner.rs` — `FeatureReadyPlanner<I: GraphInspector>` with a pure
    `classify(feature_id, bench_id) -> Result<Action, PlanError>` so the
    table is testable against `StubInspector` (PAT-001 verbatim).
  - `inspect.rs` — the new inspector methods listed below, with a
    production impl over `PlanContext` and a stub for tests.
  - `tests.rs` — unit tests over the classification table + edge cases.
- `crates/decision-cli/src/core/drive/goal.rs` — extend the `Goal` enum
  with `DefReady` and update the `Display` / parser.
- `crates/decision-cli/src/core/drive/registry.rs` — register
  `FeatureReadyPlanner` for `(Feature, DefReady)` in `planner_for`.
- Reuse of FT-111's sweep machinery: lift `SweepRow` / `SweepTally` /
  `drive_one_feature_with_timeout` into `core::drive::sweep` (currently in
  `features/ft_111_drive_ship_all/sweep.rs`) and make them generic over
  `Goal`. The lift is in scope here because PAT-003 has now reached two
  callers — this is the "migrate to core when a pattern recurs" rule
  from CLAUDE.md.

### State

- No on-disk schema change. No new artifact type.
- Reads: feature-spec metadata + body completeness, TC frontmatter +
  runner pair, dependency graph, VG coverage queries, VG review state,
  preflight projection.
- Writes (transitive, via dispatched actions only): the VGA worker may
  produce new `dec:VerificationGraph` artifacts and supersede prior
  graphs — those writes already chokepoint through FT-041 / FT-044, no
  new write paths.
- The DoR planner itself never writes to either store; like FT-110's
  driver, it is a pure coordinator.

### Behaviour

1. Parse `dec drive def-ready <FT-XXX | --all>`. Resolve `Goal::DefReady`
   from the CLI verb.
2. Look up `planner_for(ArtifactKind::Feature, Goal::DefReady)` →
   `FeatureReadyPlanner`. If the registry returns `None`, exit with the
   FT-110 `NoPlannerRegistered` error (this is a regression backstop —
   the planner should always be registered post-FT-119).
3. For single-feature mode: invoke `drive::run` with `max_iter` and the
   resolved bench. The loop is identical to FT-110's: `plan → dispatch →
   re-plan`. Only `Action::DispatchVerifyGraphAuthor` is reachable as a
   work-doing action under this goal.
4. For `--all` mode: enumerate features the same way FT-111 does
   (numeric-suffix-ascending, optional `--filter`), then for each call
   the same `drive::run` under a `per_feature_timeout`, classifying the
   per-feature result into the existing `SweepOutcome` taxonomy
   (`Done` / `Stuck` / `HitMaxIter` / `Timeout` / `Error`).

The planner's classification table (in code-comment form for spec
review):

```
spec_complete  preflight  deps_done  tcs_linked  tcs_ok  vgs_cover  vgs_accepted │  Action
─────────────────────────────────────────────────────────────────────────────────┼──────────────────────────────────
true           clean      true       true        true    true       true         │  Done
*              warnings   *          *           *       *          *            │  Stuck "preflight: <gap list>"
*              *          false      *           *       *          *            │  Stuck "blocked: <FT-Y status>"
false          *          *          *           *       *          *            │  Stuck "spec incomplete: <missing H2>"
*              *          *          false       *       *          *            │  Stuck "no TCs linked"
*              *          *          true        false   *          *            │  Stuck "TC quality: <TC-id reason>"
true           clean      true       true        true    false      *            │  DispatchVerifyGraphAuthor
true           clean      true       true        true    true       false        │  Stuck "VG pending_review: <VG-ids>"
```

Reading order: top → bottom; first matching row wins. Read this as "all
upstream gaps are stuck because they need humans, the only thing the
loop can fix on its own is missing graph coverage."

### Invariants

- `FeatureReadyPlanner::classify` is a pure function of inspector
  observations — same testability contract as
  `FeatureShipPlanner::classify` (PAT-001). No I/O, no time reads, no
  global state.
- The DoR planner never returns `DispatchImplementer` or
  `DispatchVerifier`. Those goals belong to `ship`; mixing them here
  would mean shipping the feature inside the readiness check, which
  defeats the gate.
- A `Done` outcome from `dec drive def-ready FT-XXX` is a structural
  guarantee, not a heuristic: any feature reaching `Done` will pass
  `product preflight`, `product graph check`, and the FT-045 coverage
  query for every TC in its `tests:` list. If `dec implement FT-XXX`
  subsequently fails, that's a *content* problem (worker can't write the
  code), not a *bundle* problem.
- Cycle-detection (PAT-002) is inherited unchanged: the planner reuses
  the same `state_hash_for_feature` shape, hashing the DoR-relevant
  dimensions (spec completeness, preflight bucket, TC count + per-TC
  ok-state, VG cover + accept state, dep status). A two-round
  oscillation (VGA authors a graph → gets superseded next round) yields
  a period-1 cycle and `Stuck` with a graph-theoretic reason.
- The sweep maintains FT-111's invariants verbatim: deterministic
  ordering, per-feature isolation, sequential execution, bounded total
  runtime ≤ `len(features) * per_feature_timeout + ε`.
- Generalisation of FT-111's sweep into `core::drive::sweep` does not
  change any byte of FT-111's text output for `--goal ship`. The text
  fixtures committed under FT-111's `tests/fixtures/` continue to pass.

### Error handling

- `--all` with an empty product graph or with a `--filter` that names
  unknown IDs — same behaviour as FT-111 (fail fast, exit non-zero,
  surface the unknown IDs).
- Inspector returns `Err(InspectError::StoreUnreadable)` — the
  per-feature drive surfaces as `SweepOutcome::Error { detail }` in
  `--all`; in single-feature mode the error propagates verbatim.
- A VGA dispatch returns `Err` (worker crashed, validator refused) — the
  driver does NOT swallow it (FT-110 contract); the history captures the
  failing action, and `--all` records it as `Error`.
- A `Stuck` reason carrying a TC quality complaint cites the offending
  TC's id; a preflight complaint cites the unresolved cross-cutting
  ADR(s); a dependency complaint cites the blocking feature id + its
  current status. Operators MUST be able to read the Stuck reason and
  open the right artifact without re-running anything.
- Inconsistent state where a VG is `dec:supersededBy <…>` but no
  successor exists: the inspector treats the feature as `vgs_cover:
  false` (no live graph) and the planner dispatches VGA. This is a
  cleanup, not an error.

### Boundaries

- The DoR planner does not write to the product graph (no auto-creating
  TCs, no auto-authoring spec sections, no auto-acknowledging
  cross-cutting ADRs). Authorship is a human / dedicated-author-role
  concern; the gate only observes.
- The DoR planner does not auto-accept VGs out of `pending_review`. That
  remains a Level-3 human action ([ADR-030](ADR-030)).
- The DoR planner does not run verification (does not invoke `dec verify
  feature`). Whether the *code* passes the graphs is `ship`'s problem,
  not readiness'.
- The sweep does not parallelise per-feature drives — inherited
  constraint from FT-111 (GraphWriter lock coordination is a separate
  feature).
- The CLI does not add a `--watch` mode for DoR (operators who want
  continuous monitoring use `dec drive show` per feature, per FT-113).

## Post-landing acceptance targets

Two real features in the current repo naturally exercise the live
`DispatchVerifyGraphAuthor` branch of the planner end-to-end. Both are
`status: complete`, have wired runners, and carry zero covering
`dec:VerificationGraph` artifacts on disk — exactly the shape
(`vgs_cover: false` with every other DoR bit true) the planner is
designed to convert into a single VGA dispatch followed by `Done`:

- **[FT-120](FT-120)** — primary target. TCs TC-260..TC-265 mix
  `cargo-test` and `bash` runners, so a single drive exercises both
  shell-step shapes through the VGA worker. Expected sequence for
  `dec drive def-ready FT-120 --bench BNCH-002`: iter 0
  `DispatchVerifyGraphAuthor`, iter 1 `Done`.
- **[FT-117](FT-117)** — secondary, all-bash variant. TCs TC-246..TC-248
  are all `bash` runners against `tests/scripts/tc-*.sh`. Per the
  planner-stuck mode witnessed on FT-100 (2026-06-01), a pure-bash TC
  set can surface a downstream VGA worker limitation; def-ready will
  still *classify* correctly — `DispatchVerifyGraphAuthor` is the right
  action, and any worker-side failure is out of scope for the planner.

Together with TC-254's stub-driven coverage of every `Stuck` branch
(`spec incomplete`, `preflight: …`, `blocked: …`, `no TCs linked`,
`TC quality: …`, `VG pending_review: …`), these two integration targets
give the planner full table coverage without requiring synthetic spec
drift on a real feature. After FT-119 ships, the first live verification
run should be `dec drive def-ready FT-120 --bench BNCH-002`.

## Out of scope

- **Auto-authoring missing TCs.** A future feature_spec may add a
  `tc-author` worker role + a `dec drive def-ready --author-tcs` opt-in
  for cases where the operator trusts auto-drafting. Today the bar is
  human-authored TCs; the gate just refuses to call them ready.
- **Auto-authoring missing spec sections.** Same shape as above — a
  spec-author worker is plausible long-term but is not part of this
  slice's contract.
- **Auto-resolving preflight cross-cutting gaps.** FT-104 already
  default-acknowledges via `product.toml`; gaps that *remain* after that
  are deliberate per-feature decisions that need a human.
- **`dec drive ready --all` (drop the `def-` prefix).** Naming
  inconsistency in the CLI is a worse cost than two extra characters;
  keeping the verb explicit (`def-ready`) avoids overloading the word
  "ready" with other future meanings.
- **A DoR planner over ADR-XXX / TC-XXX artifacts.** Same substrate
  applies, but the readiness rules for an ADR (proposed → accepted) or
  a TC (unimplemented → has-runner) are different state machines.
  Follow-ups, not this slice.
- **Persisting a `dec:Readiness` artifact per drive.** Tempting — every
  `Done` could materialise a snapshot — but it pre-commits a schema we
  don't yet know we'll need. The orchestration store already carries
  the constituent edges; if a future analytics feature wants a snapshot,
  it can compute one from history.
- **Integration with `dec drive ship` as an implicit precondition.**
  This was considered — making `dec drive ship FT-XXX` refuse to start
  until DoR is `Done` — but it couples two operator workflows that
  should remain composable. Operators run def-ready when they want the
  gate; ship still runs without it for power-user override and replay.
