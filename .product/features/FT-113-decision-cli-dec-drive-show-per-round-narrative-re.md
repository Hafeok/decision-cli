---
id: FT-113
title: 'decision-cli: dec drive show — per-round narrative renderer for a feature drive'
phase: 4
status: in-progress
depends-on:
- FT-110
- FT-112
adrs: []
tests:
- TC-215
- TC-216
- TC-217
- TC-218
- TC-219
- TC-220
- TC-221
- TC-259
domains:
- api
domains-acknowledged:
  api: Adds dec drive show subcommand and a read-only reader trait; cites PAT-001 for the trait-based store-read shape. No new contract beyond the documented CLI flags + JSON shape.
patterns:
- PAT-001
---

## Description

Operators have no concise live or post-hoc view of what a `dec
drive ship` run is doing. Today the choices are:

- `dec events tail` — SSE, but needs a running daemon; one-shot
  `drive ship` doesn't expose one, so tail is silent.
- `dec events since 0` — replays raw events from the
  orchestration store. Useful, but it's a wall of low-level
  ProvO + StreamWriter events; the operator has to mentally
  group them into rounds.
- `dec loop show FT-XXX` — feedback chain only; doesn't show
  which round dispatched which worker.
- Filesystem polling (`ls -lt .dec/verify/graph/`, `git log`,
  per-pane `watch`) — works but composes badly.

`dec drive show FT-XXX` reads the orchestration store +
`.dec/verify/{graph,result}/` files, groups the events into
**per-round narratives** (one round = one planner classify →
worker dispatched → observed outcome), and renders a compact,
human-readable timeline. `--watch` re-renders every 2s with a
screen clear so the same command doubles as a live dashboard
during a drive.

The expected output shape:

```
FT-111 — driving via FeatureShipPlanner — bench BNCH-002

Round 0  09:43:15  [+0s]
  state    NeverRun · 0 impl-open · 0 vga-open · 0 graphs
  dispatch verify-graph-author
  ↳ session  sess:8f3d-2c1a-…
  ↳ produced VG-167 (8 steps, covers TC-208..214)
  ↳ auto-ran 7 fail / 1 pass → 7 defects routed to implementer

Round 1  09:46:02  [+2m47s]
  state    Rejected · 7 impl-open · 0 vga-open · 1 graph
  dispatch implementer
  ↳ session  sess:c2a1-4419-…
  ↳ commit   a7b3c91 [FT-111] add SweepRow + SweepTally types
  ↳ addressed 4 of 7 defects (3 remain)

Round 2  …

Current verdict:  rejected · 3 open defects · 1 round remaining (max-iter 6)
```

The renderer is the operator's primary monitoring channel for
both running drives and historical audits. A future local-web
view (browser dashboard over all sessions and active drives) is
explicitly out of scope here — that pattern is much easier to
land once the structured reader this feature builds is
available.

## Functional Specification

### Inputs

CLI surface:

- `dec drive show FT-XXX` — render the most-recent drive's
  rounds for FT-XXX. If multiple drives exist (different
  benches, different days), default to the most recent.
- `dec drive show FT-XXX --bench BNCH-NNN` — filter to drives
  that ran on the given bench.
- `dec drive show FT-XXX --watch` — re-render every 2s with
  screen clear; exits cleanly on `Ctrl-C` / SIGINT.
- `dec drive show FT-XXX --watch --interval <secs>` — override
  the poll interval (default 2; lower bound 1).
- `dec drive show FT-XXX --since <round>` — start at round N
  (useful when the drive is long and only the tail is
  interesting).
- `dec drive show FT-XXX --format <text|json>` — `text` is the
  default human-readable narrative; `json` emits the
  `Vec<Round>` structure for tooling and tests.
- `dec drive show FT-XXX --all-drives` — show every drive run
  (separated by a divider per drive) instead of just the most
  recent.

### Outputs

- New module: `crates/decision-cli/src/features/ft_113_drive_show/`
  - `reader.rs` — pulls per-round records from the orchestration
    store + `.dec/verify/` files. Read-only; cites PAT-001's
    Inspector trait shape but adapted to the renderer's needs:
    a `DriveHistoryReader` trait with `rounds_for_feature(&self,
    feature_id, bench_id) -> Result<Vec<Round>, ReadError>` so
    tests can stub against fixed Vec<Round>.
  - `model.rs` — typed `Round` / `RoundState` / `Dispatch` /
    `Outcome` value types, `serde::Serialize` for the
    `--format json` adapter.
  - `render.rs` — pure `(rounds, options) -> String` formatter
    for text. JSON format is `serde_json::to_string_pretty(&rounds)`.
  - `watch.rs` — the `--watch` poll loop: clear screen,
    re-read, re-render, sleep, repeat. Uses `tokio::time` for
    the interval and a CtrlC handler for graceful exit.
  - `cli.rs` — adapter plumbing argparse → reader → render.
- One reference text fixture per (no rounds / mid-run / done /
  stuck) state under `tests/fixtures/` so the renderer
  contract is reviewable in PRs.

### State

Read-only. Renderer does not write any quad, does not touch
`.dec/verify/`, does not mutate any feature/TC status. The
`--watch` loop holds no in-process state beyond the rendered
string of the previous frame.

### Behaviour

1. **Read rounds.** Reader runs three SPARQL queries against
   the orchestration store:
   - All `prov:Activity` instances whose `dec:targetFeature` is
     `<feature_iri>` (gives us the dispatched sessions).
   - All `dec:VerificationGraphResult` whose `dec:resultOf`'s
     graph covers the feature's TCs (gives us per-round
     verifier output).
   - All `dec:Feedback` whose `dec:sourceArtifact` is one of
     the feature's TCs (gives us open / addressed defects per
     round).
   Plus a filesystem read of `.dec/verify/graph/*.ttl` for the
   `produced VG-NNN` line in VGA rounds.
2. **Group into rounds.** Sessions are temporally ordered;
   group consecutive sessions per dispatch event into a single
   round. A round's identity is the dispatch event timestamp.
   The reader emits `Vec<Round>` where each `Round` carries:
   - `round_index: u32` (0-based, from the dispatch sequence)
   - `started_at: DateTime<Utc>`
   - `elapsed_since_round_zero: Duration`
   - `state: RoundState { verdict, impl_open, vga_open, graph_count }`
     (the planner's observation that triggered the dispatch;
     reconstructed from the per-round VGR + feedback snapshot).
   - `dispatch: Dispatch { role, session_iri }`
   - `outcome: Outcome { ... }` (role-specific: VGA gets
     produced VG-id + auto-run pass/fail; implementer gets
     commit sha + files touched + defects addressed; verifier
     gets per-TC pass/fail; etc.).
3. **Render.** `render_text(&[Round], &RenderOpts) -> String`
   emits the layout shown in Description. Each round is a
   3–5 line block. Trailing summary line shows current verdict,
   open-defect count, max-iter remaining.
4. **Watch loop.** Every `--interval` seconds, the loop calls
   `reader.rounds_for_feature` again, re-renders, prints
   `clear-screen` + new render. Exits on SIGINT with a
   one-line "stopped" message and the final render preserved.
5. **No rounds yet.** If the reader returns an empty
   `Vec<Round>`, render a one-paragraph empty state explaining
   what to do (e.g., "No drive history for FT-111. Run `dec
   drive ship FT-111` to start one.").
6. **JSON format.** `--format json` skips the text renderer
   entirely; output is `serde_json::to_string_pretty(&rounds)`
   so downstream tooling (a future web dashboard, scripts)
   can consume the typed shape directly.

### Invariants

- The renderer is a pure function of `(Vec<Round>, RenderOpts)`.
  No side-channel reads of the store, no time.now() injected
  outside the reader. Tests pass a fixed Vec<Round> and a fixed
  current-time, and the output is byte-deterministic.
- Round order is chronological by dispatch timestamp. A round
  with index N happened strictly before N+1.
- The reader's output is unaffected by drives on other features
  in the same store; queries filter by `feature_iri` (and
  optional `bench_iri`) at the SPARQL level.
- The `--watch` loop's poll interval is bounded `[1, 60]`
  seconds; lower bounds prevent runaway store reads, upper
  bound prevents stale renders.
- Empty Vec<Round> renders the empty-state paragraph, never an
  empty buffer (operators with an empty screen can't
  distinguish "no rounds" from "render bug").
- Cross-bench drives: if multiple drives ran for the same
  feature on different benches, the default render shows the
  most recent only. `--bench` filter is exact-match;
  `--all-drives` shows everything ordered most-recent-first.

### Error handling

- Feature ID that doesn't exist in the product graph: exit
  non-zero with `"Unknown feature FT-XXX. Check `product
  feature list`."` Reader fails fast before any SPARQL query.
- Orchestration store missing / unreadable: exit non-zero with
  the underlying `Display` error and a hint to run `dec init`.
- Empty feature (no drives ever): render the empty-state
  paragraph, exit zero (this is not an error).
- A round whose state can't be fully reconstructed (e.g., the
  VGA session ended without emitting a VG IRI; this happens for
  pre-FT-068 runs): render the round with `outcome: <partial:
  reason>` and continue. Partial reconstruction is better than
  refusing to render.
- Watch loop SIGINT: render one final frame, print "stopped",
  exit zero.
- Watch loop reader error mid-run: print the error inline and
  keep polling. A transient store-lock contention shouldn't
  kill the dashboard.

### Boundaries

- The renderer does NOT dispatch or restart workers. It is
  observation-only; the operator's editing happens through
  `dec drive ship`, not through `dec drive show`.
- The renderer does NOT extend the planner's classification
  table. It re-derives the planner's observation per round
  from persisted artifacts so the operator sees what the
  planner saw; it does not call the planner.
- The text formatter is the only render target in this
  feature. A future local-web view consumes the same
  `Vec<Round>` via `--format json`, but the browser UI is its
  own feature.
- The renderer does NOT compute new aggregates over the store
  (latency histograms, worker-cost summaries, etc.); those
  belong in future analytics features.

## Out of scope

- **A local web dashboard** that visualises the full set of
  running drives, sessions, and feedback across the repo.
  Explicitly future work — the JSON format this feature ships
  is the data contract the dashboard will consume; landing the
  reader + typed shape here makes the dashboard a thin
  rendering exercise later.
- **A TUI** with ratatui-style panels. Text + watch covers the
  80% case; TUI complexity isn't justified yet.
- **Cross-feature aggregate view** (a "global drive dashboard"
  across all in-flight drives). FT-111's `dec drive ship --all`
  tally serves that need at sweep completion; live aggregate
  monitoring would compose `dec drive show` per feature
  externally for now.
- **Color / ANSI styling**. The text format ships
  ASCII-and-Unicode-glyphs-only so it's diff-able, logged,
  pasted into chat, etc. Color is a future ergonomics layer.
- **Resume / persist the watch loop's render state across
  invocations.** Each `--watch` start is a fresh poll; no
  state file. The orchestration store is the only persistent
  source.
- **Editing the rendered output.** No interactive controls,
  no keystroke filters. Pure display.
