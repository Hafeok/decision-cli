---
id: FT-135
title: 'decision-cli: dec drive live progress feedback (per-round + per-worker stderr stream)'
phase: 4
status: complete
depends-on:
- FT-110
- FT-111
- FT-119
adrs: []
tests:
- TC-324
- TC-325
- TC-326
- TC-327
domains:
- api
- observability
domains-acknowledged:
  api: Extends the existing `dec drive *` surface with a `--quiet`/`-q` flag and a documented stderr line format. No new verbs, no new artifact contract, no MCP twin needed — progress is a runtime concern, not a graph mutation. Honours ADR-011 (single-command CLI shape) by keeping every variant of `dec drive` consistent in flag set and line format.
  observability: 'Adds structured progress events emitted via the existing `tracing` crate at `target: "dec::drive::progress"`, mirrored to stderr through a `ProgressSink` trait. Reuses the same tracing infrastructure that already covers the escalation paths (`features/drive/execute.rs:77,104`) and the inspector warnings (`features/drive/inspect.rs:746`). Does not introduce a new sink, formatter, or filter syntax — operators tune verbosity with `RUST_LOG` exactly as today.'
---

## Description

Today `dec drive ship FT-XXX`, `dec drive def-ready FT-XXX`, and `dec drive def-ready --all` are **silent during execution**. The operator sees nothing while the driver iterates — no per-round planner decision, no "dispatching verify-graph-author for FT-119" line, no worker exit code, no elapsed time. Output appears only at the terminal `Done`/`Stuck` boundary, after the history is dumped in one go.

For single-feature drives this is mildly annoying — a 30-second `def-ready` looks like a hang and the operator can't tell whether the planner is between rounds, a worker is running, or something is wedged. For `--all` sweeps it is materially harmful: nothing prints until the *entire* sweep completes. A 30-feature sweep with one 5-minute worker dispatch in the middle blocks the operator for 5+ minutes with no signal that progress is happening, no indication of which feature is currently in flight, and no way to interrupt confidently (Ctrl-C might be aborting a mid-flight worker that just needs a few more seconds).

This feature adds **live progress feedback** to every `dec drive` verb so the operator sees what the driver is doing, in roughly the same way `git rebase`, `cargo build`, and `kubectl rollout status` narrate their work. The minimum viable shape is **structured stderr lines, one per planner round and per worker dispatch**, plus **per-feature outcomes streamed incrementally** for `--all` sweeps. Quiet-by-default is *not* the right default here — the existing surface already prints nothing useful while running; the regression risk for scripts is small (terminal `Done` / `Stuck` history dump still goes to stdout unchanged) and the upside for interactive use is large. A `--quiet`/`-q` flag preserves the old behaviour for headless / CI use.

The feedback shape is constrained by the surrounding architecture, not invented from scratch:

- **Where progress lives.** The driver loop in `features/drive/run.rs:67` already has a clean per-iteration seam — it sees the planner decision, executes the action, and records a `HistoryEntry`. The natural insertion point is a `ProgressSink` trait threaded through the loop, mirroring the existing `Executor` trait test seam. Production wires a `StderrProgressSink`; tests wire a `RecordingProgressSink` that captures lines for assertions.
- **What format.** Single-line, machine-friendly, tab-separated key=value pairs prefixed by feature id. Compatible with `grep`, `awk`, and `tee | jq` (with a `--format json` follow-on if needed; out of scope for this slice).
- **How it composes with tracing.** Every line also goes through `tracing::info!(target: "dec::drive::progress", ...)` so operators who already filter with `RUST_LOG=dec::drive=trace` get the same data structured. The stderr sink is the human surface; tracing is the machine surface.
- **Sweeps stream per-feature.** `ft_111_drive_ship_all::run_sweep` collects rows then renders at the end (`cli/drive.rs:387`). The fix is to emit each row to stderr immediately upon completion *in addition to* including it in the final summary table.

After this feature, `dec drive def-ready --all` running against the current catalogue would look something like:

```
[FT-119] iter 0  plan=DispatchVerifyGraphAuthor env=BNCH-002
[FT-119] iter 0  exec start: verify-graph-author
[FT-119] iter 0  exec ok    8.3s
[FT-119] iter 1  plan=Done
[FT-119]         outcome=Done iter=1 elapsed=8.6s
[FT-104] iter 0  plan=DispatchVerifyGraphAuthor env=BNCH-002
[FT-104] iter 0  exec start: verify-graph-author
[FT-104] iter 0  exec ok    7.9s
[FT-104] iter 1  plan=DispatchVerifyGraphAuthor env=BNCH-002    ← cycle detector will fire next round
[FT-104]         outcome=Stuck reason="dispatch:verify-graph-author dispatch did not change state for FT-104"
[FT-125]         outcome=Stuck reason="blocked: FT-123 status=planned"
...
Feature Sweep Results              ← final summary still prints to stdout
=====================
FT-119      1022ms    0 iter  ✓ Done
FT-104     3958ms    0 iter  ✗ Stuck: dispatch:verify-graph-author dispatch did not change state for FT-104
...
```

The summary table is unchanged. Stderr carries the live narration; stdout carries the structured final result.

## Functional Specification

### Inputs

#### New CLI surface — flags on every `dec drive` subcommand

```
--quiet, -q   Suppress per-round and per-worker progress lines on stderr.
              The terminal Done/Stuck history dump and the --all summary
              table on stdout are unaffected. Default: false (progress on).
```

The flag is added to every existing `dec drive` subcommand (`ship`, `def-ready`, `show`). `dec drive show` is post-hoc and emits nothing during planning, but accepting `--quiet` keeps the flag set uniform across the verb family per ADR-011.

#### `RUST_LOG` tuning

Progress events are emitted at `tracing::Level::INFO` with `target = "dec::drive::progress"`. Operators who want even more detail keep using `RUST_LOG=dec::drive=debug` exactly as today; nothing changes about how tracing is configured.

### Outputs

#### Per-round line (stderr)

Emitted once per planner iteration before execution begins:

```
[FT-XXX] iter N  plan=<ActionTag> [<key>=<value>]...
```

`<ActionTag>` is `Action::tag()` (the existing variant tag — `Done`, `Stuck`, `DispatchVerifyGraphAuthor`, etc.). Optional `key=value` trailers carry variant-specific context: `env=BNCH-NNN` for VGA dispatches, `reason="..."` for Stuck, none for Done.

#### Per-execution lines (stderr)

Bracket every non-terminal action with start + end lines:

```
[FT-XXX] iter N  exec start: <action-tag>
[FT-XXX] iter N  exec ok    <elapsed>s
[FT-XXX] iter N  exec fail  <elapsed>s  err="<detail>"
```

Elapsed is wall-clock, two-decimal seconds. The `err=` trailer is quoted and tab-free.

#### Per-feature outcome line (stderr, sweeps only)

When a feature terminates inside `--all`, immediately:

```
[FT-XXX]         outcome=<Done|Stuck|MaxIter|Error> iter=N elapsed=Ts [reason="..."]
```

The `outcome=` line streams *before* the next feature begins, so the operator sees forward progress through the sweep in real time. The eventual summary table on stdout still aggregates everything.

#### Stdout — unchanged

Terminal `Done`/`Stuck` history dumps for single-feature drives and the `Feature Sweep Results` table for `--all` continue to go to stdout in the exact format they have today. Scripts that consume `dec drive` output are unaffected.

### State

- **No new persisted state.** Progress lines are emitted and forgotten; the graph and orchestration store are not touched.
- **No new event types in oxi-events.** A future slice may bridge progress into the SSE stream consumed by `dec events tail` (so a remote observer sees the same narration); explicitly out of scope here.
- **No new public types in `core/drive/`.** The `ProgressSink` trait lives in `features/drive/` alongside `Executor`. Core stays substrate-only per the slice-level SDP rule.

### Behaviour

#### Wiring inside the driver loop

`features/drive/run.rs::run_with_planner_and_executor` (the loop body) grows one extra parameter: `progress: &dyn ProgressSink`. On each iteration:

1. Call `progress.on_plan(feature_id, iter, &action)` before the variant match.
2. On non-terminal `other`, call `progress.on_exec_start(feature_id, iter, action_tag)` immediately before `executor.execute(...)`.
3. Time the execute call; on return call `progress.on_exec_end(feature_id, iter, action_tag, elapsed, result)` regardless of outcome.
4. On terminal `Done` / `Stuck` / `MaxIterations`, call `progress.on_outcome(feature_id, &outcome_or_err)` before the function returns.

The public `run(ctx, args)` entry point keeps its current signature and wires a `StderrProgressSink` internally. A new `run_with_progress(ctx, args, progress)` entry point is added for callers that need a different sink (the sweep code uses this to mux per-feature progress into a single shared sink).

#### `--quiet` resolution

`StderrProgressSink::new(quiet: bool)` — when `quiet == true`, every callback is a no-op. The trait method calls still happen (so `tracing` still fires); only the stderr writes are suppressed.

#### Sweep streaming

`ft_111_drive_ship_all::run_sweep` already iterates features in a Tokio task per feature. The fix is to thread a shared `Arc<StderrProgressSink>` into each feature's `run_with_progress` call, plus emit the per-feature `outcome=` line at the point the task records its row into the shared rows vector (so it streams as it happens, not after the join).

#### Format determinism

Stderr lines are byte-deterministic for a given (feature_id, iter, action_tag, env_id) tuple modulo the elapsed time. Tests assert this by recording lines into a `RecordingProgressSink` and matching against fixture strings with the elapsed field masked.

#### Worker stdout / stderr pass-through

Workers (`verify-graph-author`, `code-writer`, `verify-feature`) print their own diagnostics. This feature does **not** intercept or reformat worker output — it brackets the dispatch with `exec start` / `exec ok` lines. The worker output is left to interleave on stderr as it does today; operators who want it muted run with `RUST_LOG=warn` or pipe `2>/dev/null` (and accept losing the progress lines). A future slice could capture worker stderr and prefix it; out of scope here.

### Invariants

- **stdout is unchanged.** Every existing line that goes to stdout (single-feature history dump, sweep summary table, `dec drive show` narrative) continues to go to stdout in byte-identical form. The only difference visible to a stdout-only consumer is timing: with progress streaming, stdout still appears at the same end-of-drive boundary.
- **`--quiet` is the only way to suppress progress.** `RUST_LOG=off` does not suppress stderr writes from the sink (tracing and the sink are independent). This is intentional — operators expect `--quiet` to mean quiet, not "configure six env vars".
- **No interleaving guarantees across features.** Sweep tasks emit concurrently; lines from different features may interleave at the line boundary. The `[FT-XXX]` prefix makes the lines greppable; preserving inter-feature ordering would require serialising the sink and slowing the sweep down, which is the wrong tradeoff.
- **Progress sink is infallible.** Stderr write errors are swallowed (the sink is `pub fn on_xxx(...)`, not `Result`). A drive must not fail because a pipe was closed. Tracing handles its own errors per the `tracing` crate's contract.
- **`ProgressSink` trait stays in `features/drive/`.** It is a feature-level concept (the driver-loop seam), not core substrate. The SDP boundary stays intact.
- **The driver loop's existing semantics are unchanged.** `max_iter` counting, cycle detection (PAT-002), terminal action handling — all proceed exactly as today. Progress emission is a side-channel observer; it cannot affect planning decisions.

### Error handling

- **Stderr pipe closed / broken pipe.** Sink swallows the error and continues. The drive completes normally; downstream of the broken pipe simply sees a truncated narration.
- **Tracing subscriber not initialised.** `tracing::info!` is a no-op when no subscriber is attached; the stderr sink still fires. Operators running `dec` outside the configured environment still see progress.
- **Worker dispatch panics.** The existing `Executor::execute` contract is unchanged; a panic propagates as today. The `on_exec_end` callback runs in a `Drop` guard pattern so an `exec fail` line is emitted even if the executor unwinds.
- **`--quiet` parsing collisions with planner-level args.** None: `--quiet`/`-q` is a top-level `dec drive *` arg, not consumed by planners.

### Boundaries

- **In scope.** `ProgressSink` trait + `StderrProgressSink` production impl + `RecordingProgressSink` test impl; threading the sink through `run_with_planner_and_executor` and the `ft_111_drive_ship_all` sweep; `--quiet`/`-q` flag on `ship`, `def-ready`, `show` subcommands; per-round / per-exec / per-outcome line formats as specified above; tests for line format + `--quiet` suppression + sweep streaming order.
- **In scope (small).** Documenting the line format in `dec drive --help` text so operators discover the contract without reading source.
- **Out of scope.** SSE bridge so remote `dec events tail` observers see the same lines (graph-native progress is a future slice; this feature is the local stderr surface). Capturing and prefixing worker stdout/stderr (a related but distinct concern — worker outputs are noisy, structured differently per worker, and re-wrapping them is a separate feature). Spinner / TUI / progress bar UI (not the right shape for a CLI that's often piped). `--format json` for progress lines (the existing tsv-ish format is enough for slice 1; jq-friendly JSON can come later if anyone asks). Quiet-by-default (an operator-facing default change of this kind needs more usage data first). Per-iteration timing histograms / aggregation (out of slice; `dec drive show` already covers post-hoc analysis). Capturing progress to a file by default (operators can redirect stderr).

## Out of scope

- SSE / graph-event integration for remote progress observers.
- Worker stdout/stderr capture or reformatting.
- TUI, spinner, or progress-bar rendering.
- `--format json` for progress lines.
- Quiet-by-default behaviour change.
- Post-hoc timing aggregation / histograms (covered by `dec drive show`).
- Cross-feature line ordering guarantees inside `--all` sweeps.
