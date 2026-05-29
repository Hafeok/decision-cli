---
id: FT-111
title: 'decision-cli: dec drive ship --all multi-feature sweep'
phase: 4
status: planned
depends-on:
- FT-110
adrs: []
tests:
- TC-200
- TC-201
- TC-202
- TC-203
- TC-204
- TC-205
- TC-206
- TC-207
domains:
- api
domains-acknowledged:
  api: Sweep is a thin loop around FT-110's per-feature drive — no new API surface beyond the documented CLI flags. The api-domain ADRs (ADR-043, ADR-047 etc.) govern the slice + adapter shape this feature inherits from PAT-001 without modification
patterns:
- PAT-001
- PAT-002
- PAT-003
---

## Description

Add `dec drive ship --all` to the CLI: a single command that
iterates every product feature, runs the existing FT+Ship driver
loop per feature with a per-feature timeout, and reports a
structured tally. The current operator workflow is a bash script
(`scripts/dogfood_all_features.sh`) that loops `dec drive ship
FT-XXX` and tallies outcomes with `awk`. Two sweeps in the last
session surfaced real planner/matcher bugs from the tally — the
shape has graduated from one-off experiment to regular operator
action, and the script's `bash + awk + timeout 600` ancestry
should fold into the CLI for parity with every other operator
command.

The sweep reuses FT-110's `FeatureShipPlanner` per feature
(per PAT-001) and inherits PAT-002's cycle detection for free
(the planner is the one being iterated). The new shape is
PAT-003: per-item bounded execution, per-item failure isolation,
deterministic ordering, pure formatter over a typed (rows,
tally) pair.

An optional `--retire-failing-graphs` pre-pass replaces the
bash script's unconditional SPARQL-based supersession of every
non-approved graph before each per-feature drive. The pre-pass
is opt-in in the CLI (off by default) because silently mutating
the orchestration store on every invocation would surprise any
operator deliberately keeping a failing graph around for
investigation.

## Functional Specification

### Inputs

CLI surface (added to the existing `dec drive ship` subcommand):

- `dec drive ship --all` — sweep every feature.
- `dec drive ship --all --env <ENV-XXX>` — env override; defaults
  to the workdir's default env (matches single-feature behaviour).
- `dec drive ship --all --max-iter <N>` — per-feature iteration
  cap; defaults to 6 (matches the recently-tuned sweep value
  where the state-hash cycle detector has room for period ≤ 5).
- `dec drive ship --all --per-feature-timeout <secs>` — bound on
  any one feature's drive loop; defaults to 600s.
- `dec drive ship --all --retire-failing-graphs` — opt-in
  pre-pass that supersedes every non-approved
  non-already-superseded VG covering each feature before running
  the driver. Off by default.
- `dec drive ship --all --format <text|tsv|json>` — output
  shape; defaults to `text`.
- `dec drive ship --all --filter <FT-XXX,FT-YYY,...>` — optional
  comma-separated allowlist; if absent, every feature is in
  scope. Useful for narrow re-runs ("just the 12 that were stuck
  yesterday").

`dec drive ship FT-XXX` (the existing single-feature form) is
unchanged. `--all` and `FT-XXX` are mutually exclusive at the
argument parser level.

### Outputs

- New module: `crates/decision-cli/src/features/ft_111_drive_ship_all/`
  containing the sweep logic per PAT-003 (`sweep.rs` for the loop,
  `format.rs` for the text/tsv/json renderer, `cli.rs` for the
  argument plumbing, `tests.rs` for unit tests).
- Extension to `crates/decision-cli/src/cli/drive.rs` (or
  wherever the existing `dec drive ship` adapter lives): one
  branch that routes `--all` to the new sweep entry point and
  passes existing single-feature args through unchanged.
- Per-feature row type (`SweepRow`) and aggregate tally type
  (`SweepTally`), both `serde::Serialize`, exposed pub(crate) so
  tests can assert on the shape independently of the formatter.
- Three reference render fixtures under `tests/fixtures/` (one
  per format).

### State

The sweep is stateless in the planner-state sense — each
per-feature drive run keeps its own ring buffer (PAT-002), and
the sweep itself only accumulates `Vec<SweepRow>` in memory.
The orchestration store is mutated only as a side-effect of the
per-feature drive (and, when `--retire-failing-graphs` is set,
by the pre-pass via the existing `supersede_graph` helper).

### Behaviour

1. **Resolve feature set.** Read the product graph for every
   feature_spec; produce a `Vec<String>` of feature IDs sorted
   ascending by numeric suffix (FT-3 before FT-10). If `--filter`
   is set, intersect against it (preserve sorted order). If the
   intersection is empty, fail fast with a non-zero exit code
   and a message naming the unknown IDs.
2. **(Optional) Retire stale graphs.** If
   `--retire-failing-graphs` is set, run the bash script's
   existing SPARQL — graphs in the env whose latest VGR is not
   `approved` and which are not already superseded — and call
   `supersede_graph` against each, marking them
   `dec:supersededBy <urn:dec:retired-stale-sweep-{ts}>`. Log
   "retired N stale graphs" per feature to the detail stream.
3. **For each feature, drive ship under a timeout.** Wrap the
   existing `drive_ship_one_feature(ctx, ft, env, max_iter)`
   call in `tokio::time::timeout(per_feature_timeout, ...)`.
   Classify the result into a `SweepOutcome`:
   - `Done` — driver reported reached goal.
   - `Stuck { reason }` — driver returned `DriveError::Stuck`
     (any reason — pairwise no-progress, escalation exhausted,
     state-hash cycle).
   - `HitMaxIter` — driver returned `DriveError::MaxIter`.
   - `Timeout { after_secs }` — `tokio::time::timeout` fired.
   - `Error { detail }` — any other error (store unreadable,
     resolver error, etc.); the detail string is the `Display`
     of the underlying error.
4. **Continue past per-feature failures.** Any single feature's
   outcome — including `Error` — never aborts the sweep. The
   row is recorded and iteration continues.
5. **Build the tally from the rows.** After the loop, compute
   `SweepTally` by counting per-outcome buckets. Derive it from
   the rows (don't track in parallel) so a future panic at the
   row-construction boundary surfaces as a tally undercount and
   not a silent disagreement.
6. **Render.** Pass `(rows, tally)` to the formatter selected by
   `--format`. The formatter is a pure function `(rows, tally,
   Format) -> String`; it does not touch the store.
7. **Exit code.** Zero if every feature was `Done`, otherwise
   one. Operators chain `dec drive ship --all && ...` for
   gate-style use.

### Invariants

- The per-feature drive runs sequentially, not in parallel.
  Parallelism would require GraphWriter lock coordination across
  features and is out of scope for the first cut.
- The resolver's feature ordering is deterministic and stable
  across runs (numeric suffix ascending). Two invocations with
  the same store state produce identical row order, byte-for-byte
  identical text/tsv/json (modulo timestamps in the row's
  `elapsed_ms`).
- Per-feature ring buffers (PAT-002) are independent: the planner
  instance for FT-A's drive does not leak hashes into FT-B's
  drive. The simplest enforcement is "construct a fresh planner
  per feature"; alternative is the per-feature-id reset already
  in PAT-002 — either works.
- The retire pre-pass writes only `dec:supersededBy` edges; no
  graph file is deleted from `.dec/verify/graph/`. Stale graphs
  remain auditable on disk after the sweep.
- The CLI always exits within
  `len(features) * per_feature_timeout + ε`. No background tasks
  outlive the process.

### Error handling

- Empty feature set with no `--filter`: exit non-zero with
  "no features in the product graph; run `product feature new`
  first". (Operator deserves a hint, not an empty `rows: []`.)
- `--filter FT-XXX` where FT-XXX doesn't exist: exit non-zero
  before any drive runs, naming the unknown IDs.
- Per-feature drive that returns an unexpected error not in the
  outcome taxonomy: catch and reify as `Error { detail }`. Never
  panic across the iteration boundary.
- Resolver error (orchestration store unreadable, product graph
  missing): fail the whole sweep before iterating. The pre-pass
  has the same failure mode — if SPARQL against the store
  errors, the sweep aborts (this is real store corruption, not a
  per-feature issue).

### Boundaries

- The sweep does not introduce a new dispatch path; it composes
  the existing per-feature ship driver.
- The sweep does not change the planner's behaviour in any way.
  PAT-002's cycle detection runs per-feature, identically to the
  single-feature `dec drive ship FT-XXX` invocation.
- The sweep does not write to the product graph
  (`.product/features/...`). It only reads.
- The retire pre-pass writes only to the orchestration store
  (`.dec/store/orchestration.nq`). No on-disk .ttl files are
  altered.

## Out of scope

- **Parallel per-feature drive.** Reasonable next iteration once
  store-write coordination is built (multi-writer GraphWriter
  with named-graph isolation, or a dispatch lease abstraction).
  Not required for the first useful version.
- **Resume / checkpoint of an interrupted sweep.** Operators can
  re-run with `--filter` to pick up where they left off; that's
  enough for now.
- **Watch-mode (sweep on a recurring timer).** Belongs in a
  future `dec schedule` integration, not here.
- **Differential / git-aware feature filtering** ("just the
  features touched by HEAD~1..HEAD"). Useful but a different
  axis of selection; can ship later as an additional `--filter`
  source.
- **Direct deletion of stale graphs on disk.** The pre-pass
  marks graphs `supersededBy`; physical cleanup of
  `.dec/verify/graph/*.ttl` files is a separate housekeeping
  concern.
