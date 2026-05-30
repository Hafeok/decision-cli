---
id: PAT-003
title: Multi-artifact sweep with per-item bounded execution and structured tally
status: live
domains:
- api
adrs: []
requires:
- PAT-001
examples:
- FT-111
- FT-119
---

## When to use

Any operator-facing CLI shape of the form "do <X> for every
<artifact> matching <filter>." The first instance is `dec drive
ship --all`, which will iterate every product feature, run the
ship driver per feature, and emit a tally. The same shape will
recur for `dec verify --all`, future periodic-meta-loop tools,
and any future "rotate through the work queue" command.

The pattern's value is that it imposes one shared discipline on
every multi-item command — per-item timeout, per-item failure
isolation, configurable report format, deterministic ordering —
so operators learn the shape once and the codebase only debugs
the iteration scaffold once.

## Prerequisites

- **PAT-001** — Inspector + Planner trait pair. The per-item op
  inside a sweep is almost always a single-item planner-driven
  dispatch; the sweep is a loop around it. Without PAT-001 the
  sweep ends up importing concrete store access alongside the
  iteration scaffold and the two become impossible to test
  independently.
- Familiarity with `tokio::time::timeout` for per-item bounded
  execution, and with `serde::Serialize` for the per-item row +
  tally types (the report formatter consumes them).

## The pattern

Four pieces: a resolver that produces the artifact list, a
per-item runner that runs the single-item op with a timeout, an
aggregator that collects per-item rows + outcome tallies, and a
formatter that renders text / tsv / json over the same data.

```rust
// crates/decision-cli/src/features/ft_NNN_drive_ship_all/sweep.rs

#[derive(Debug, Clone, serde::Serialize)]
pub enum Outcome {
    Done,
    Stuck { reason: String },
    HitMaxIter,
    Timeout { after_secs: u64 },
    Error  { detail: String },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Row {
    pub feature_id: String,
    pub outcome: Outcome,
    pub iterations: u32,
    pub elapsed_ms: u64,
}

#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Tally {
    pub done: usize,
    pub stuck: usize,
    pub max_iter: usize,
    pub timeout: usize,
    pub error: usize,
}

pub struct SweepInput<'a> {
    pub features: Vec<String>,       // resolver supplies; pre-sorted deterministically
    pub env_id: &'a str,
    pub max_iter: u32,
    pub per_item_timeout: Duration,
}

pub async fn run_sweep(
    ctx: &PlanContext,
    input: SweepInput<'_>,
) -> Result<(Vec<Row>, Tally), SweepError> {
    let mut rows = Vec::with_capacity(input.features.len());
    let mut tally = Tally::default();
    for ft in &input.features {
        let started = Instant::now();
        let outcome = match tokio::time::timeout(
            input.per_item_timeout,
            drive_one(ctx, ft, input.env_id, input.max_iter),
        ).await {
            Ok(Ok(result))                => classify_result(result),
            Ok(Err(DriveError::Stuck{r})) => Outcome::Stuck { reason: r },
            Ok(Err(DriveError::MaxIter))  => Outcome::HitMaxIter,
            Ok(Err(e))                    => Outcome::Error { detail: e.to_string() },
            Err(_elapsed)                 => Outcome::Timeout {
                after_secs: input.per_item_timeout.as_secs(),
            },
        };
        bump(&mut tally, &outcome);
        rows.push(Row {
            feature_id: ft.clone(),
            outcome,
            iterations: /* drive_one returns this */,
            elapsed_ms: started.elapsed().as_millis() as u64,
        });
    }
    Ok((rows, tally))
}
```

A formatter consumes `(rows, tally)` and emits the requested
shape — text (human-readable), tsv (one row per feature), json
(`{ "rows": [...], "tally": {...} }`). The formatter is pure; the
sweep returns data, not strings.

Five disciplines this pattern enforces:

1. **Per-item failure isolation.** One failing feature never
   aborts the sweep. Outcomes that would normally bubble as
   `Err` get caught and reified into an `Outcome::Error` row.
2. **Per-item timeout via `tokio::time::timeout`.** The cap is a
   sweep-level argument (not a hardcoded `timeout 600`); the
   shell-script ancestor of this pattern wrapped the binary in
   `timeout 600` which is exactly what `tokio::time::timeout`
   provides natively, with the bonus that the per-item iteration
   count comes back even on timeout.
3. **Deterministic ordering.** The resolver returns artifacts in
   a defined order (alphabetical, by ID-suffix, by dependency
   topological — pick one and stick to it); the sweep iterates
   in that order; the output preserves it. Operators can
   `diff` two sweep outputs directly.
4. **Per-row structured data, formatter pure.** `rows` is
   `Vec<Row>`; the formatter is `fn(rows, tally, Format) ->
   String`. Adding `--format markdown` is a formatter change, not
   a sweep change.
5. **Tally is derivable from rows.** Don't track tally during the
   loop without also producing the row; in the rare case the row
   gets dropped (e.g. a panic at the resolve boundary), tally
   would silently undercount. Build tally from rows at the end if
   you can.

## Anti-patterns

- **Shell-script as the sweep.** A bash loop around `dec drive
  ship FT-XXX` is a fine first draft but graduates badly: every
  operator that wants the same shape ends up with a fork of the
  script, the timeout/retry/report semantics diverge, and the
  whole thing's invisible to `dec --help`. Promote to a CLI
  command as soon as the shape recurs.
- **Wrapping the per-item op in a thread instead of
  `tokio::time::timeout`.** `std::thread::spawn` + `join_handle.
  join_timeout` is the wrong shape for a tokio-based driver
  loop; the executor is already async, and `tokio::time::timeout`
  cooperates with cancellation. Threads also lose the per-item
  iteration count on timeout.
- **Aborting the sweep on first failure.** A sweep's job is to
  report state across the whole set; one stuck feature is data,
  not an error. Use `Outcome::Error` and continue.
- **Reading `Format::Text` in the sweep itself.** The sweep
  returns `(rows, tally)`; the adapter chooses the format.
  Mixing rendering into the sweep makes a second formatter
  impossible without forking the iteration.
- **Pre-pass that mutates state without a flag.** The shell
  script always retires stale failing graphs before each
  drive ship. The CLI equivalent must gate this behind
  `--retire-failing-graphs` (or similar): a sweep that silently
  mutates the orchestration store on every invocation is a
  hard-to-debug surprise the first time it eats a graph an
  operator was deliberately keeping around.

## Worked example

`dec drive ship --all` (the feature this pattern was authored
for; see the feature_spec for the exact CLI surface). It
resolves the feature set via the existing product-cli reader,
runs `FeatureShipPlanner` per feature with `tokio::time::timeout`,
collects `Vec<Row>` + `Tally`, and emits text / tsv / json based
on `--format`. The retire-stale-graphs pre-pass is gated behind
`--retire-failing-graphs` (off by default; the shell-script
ancestor ran it unconditionally and that was load-bearing for
the test workflow, but unconditional state mutation in the CLI
is wrong by default).

Operationally: the prior shell sweep took ≈40 minutes for 110
features and produced a TSV; the CLI sweep does the same work
with deterministic ordering, per-item timing, and a single tally
line at the end — invocable from `dec --help` like any other
operator action.
