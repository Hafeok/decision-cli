---
id: FT-099
title: 'decision-cli: dec verify graph run + dec verify feature (CLI + MCP)'
phase: 3
status: complete
depends-on:
- FT-097
- FT-098
adrs:
- ADR-028
- ADR-029
tests:
- TC-158
- TC-159
- TC-160
- TC-161
domains: []
domains-acknowledged: {}
---

## Description

Two user-facing entry points for the slice-3 graph runner, both with paired MCP tools per [ADR-029](ADR-029):

- `dec verify graph run <VG-NNN>` / `dec_verify_graph_run` — execute a single `VerificationGraph` against its declared environment via [FT-098](FT-098)'s `core::verify::run_graph` handler. Renders the per-step trace as it streams and prints the final per-graph verdict plus the path to the persisted `VGR-NNN.ttl` ([FT-097](FT-097)).
- `dec verify feature <FT-NNN>` / `dec_verify_feature` — roll-up entry: run every `VerificationGraph` whose `dec:verifies` points at the feature (or whose covered TCs intersect the feature's TCs), then compose the per-graph results through [FT-097](FT-097)'s aggregation function and print the aggregate verdict per TC and per feature. This is the verb the operator actually wants most of the time ("is this feature done?").

Both verbs share the same handler chain underneath: bundle assembly → graph enumeration → per-graph `run_graph` invocation → aggregation (single-graph for `graph run`, full multi-graph for `feature`). Single-handler discipline per [ADR-029](ADR-029) — the CLI and MCP routes converge on one `run_handler` and one `feature_handler`.

One subcommand → one slice — this slice covers the two verbs and their MCP twins. The auto-dispatch subscription that fires the same handlers from events is [FT-100](FT-100). The chain-integrity gate ([FT-047](FT-047)) is a separate consumer of [FT-097](FT-097)'s aggregation function and is not modified in this slice.

## Functional Specification

### Inputs

#### `dec verify graph run`

```
dec verify graph run <VG-NNN>
    [--capture name=value]...                  # pre-seeded capture bindings (repeatable)
    [--format text|json|sse]                   # default text
    [--no-feedback]                            # skip Feedback emission on failure (CI/diagnostic mode)
    [--keep-tmp]                               # set DEC_KEEP_TMP=1 for env teardown debugging
```

MCP twin `dec_verify_graph_run` — input `{ graph_id: string, capture_bindings?: dict, no_feedback?: bool }`. Streaming output via the standard MCP `progress` channel matches `--format sse`.

#### `dec verify feature`

```
dec verify feature <FT-NNN>
    [--environment <ENV-NNN>]                  # filter to one env; default: all envs with covering graphs
    [--format text|json]                       # default text
    [--no-feedback]
    [--include-stale]                          # consider VGRs older than 24 h (default: re-run instead)
    [--dry-run]                                # enumerate which graphs would run; do not execute
```

MCP twin `dec_verify_feature` — input `{ feature_id: string, environment_id?: string, include_stale?: bool, dry_run?: bool }`.

### Outputs

#### `graph run` — text format

```
Running VG-067 (verifies FT-079) in ENV-002 (ephemeral-cli)
  [0] shell-command     pass   8 ms    mkdir -p .dec/store
  [1] shell-command     pass   3 ms    touch .dec/store/bundle.ttl
  [2] sparql-assertion  fail  42 ms    expected 1 row, got 0      → TC-144

Verdict: rejected
Rationale: step 2 (sparql-assertion) failed: expected 1 row, got 0; 1 TC affected
Result:    .dec/verify/result/VGR-018.ttl
Feedback:  FB-031  → TC-144 (class=regression)
```

#### `feature` — text format

```
Feature FT-079 (pipeline-worker SDK: curated query helpers...)
  Environments with covering graphs: ENV-002

  VG-067 (ENV-002)   → rejected   step 2 fail
  VG-073 (ENV-001)   → approved   3/3 pass

  Per-TC verdict:
    TC-144  rejected    (covered by VG-067 fail / no other coverage)
    TC-145  approved    (covered by VG-073 pass)

  Coverage gaps: none

Aggregate verdict: rejected
Rationale:        TC-144 rejected — 1 of 2 TCs covered by failing graphs
```

#### `--format json` (both verbs)

Returns the full `RunGraphResponse` / aggregate verdict structure as JSON for scripting. SSE format streams per-step events as they are produced (one event per `VerificationStepTrace` write).

#### Exit codes

| Outcome | Exit |
|---|---|
| Aggregate `verdict = approved` (or single-graph `verdict = approved`) | 0 |
| `verdict = amendment-required` | 2 |
| `verdict = rejected` | 1 |
| Handler error (graph not found, env unresolvable, persistence failure) | 1 with `Error::*` on stderr |
| Coverage gap on `dec verify feature` (TCs with no covering graph) | 3 (distinct so CI can branch) |

### State

- Reads: graphs, envs, TCs, prior `VerificationGraphResult` artifacts (for `--include-stale` and the dry-run enumerator).
- Writes: delegates entirely to [FT-098](FT-098) — this slice does not write `VGR` or `FB` artifacts directly. It does write the **`Session` artifact** for the run (status `running` → `completed` / `failed`) per the existing slice-2 session-record conventions, so the run shows up in `dec session list` and threads into PROV-O.

### Behaviour

#### `dec verify graph run` handler

1. Resolve `VG-NNN`. Missing → `Error::ArtifactNotFound`; exit 1.
2. Open a `Session` artifact with role `verify-graph-runner`, `status = running`, `prov:wasGeneratedBy = <activity>`.
3. Build a `RunGraphRequest` with `triggered_by = TriggerKind::Manual`, the supplied `capture_bindings`, the new `run_activity` IRI.
4. Invoke `core::verify::run_graph(request)`. Stream per-step traces to the renderer as they are written (the executor exposes a `tokio_stream::Stream<Item = StepTraceEvent>` alongside the final return for renderers that want streaming; the text renderer drains it before printing the verdict block).
5. On return: render the trace + verdict + result path + emitted feedback IRIs. Close the session as `status = completed` and pin the result IRI on the session.
6. Map verdict → exit code per the table above.

#### `dec verify feature` handler

1. Resolve `FT-NNN` and its `dec:tests` (the TCs). Missing → `Error::ArtifactNotFound`; exit 1.
2. Enumerate covering graphs: SPARQL `SELECT ?vg ?env WHERE { ?vg dec:verifies <FT-NNN> } UNION { ?vg dec:steps/rdf:rest*/rdf:first/dec:providesEvidenceFor ?tc . <FT-NNN> dec:tests ?tc }`. Filter to `--environment` if set.
3. **Freshness check.** For each `(vg, env)`, look up the latest `VerificationGraphResult` by `dcterms:created`. If it is older than the freshness window (24 h, configurable via `--include-stale` or `.dec/config.toml`'s `[verify_feature] freshness_hours = 24`), schedule a re-run. Otherwise reuse the existing VGR.
4. `--dry-run` → render the enumeration (which VGRs would run, which would be reused) and exit 0.
5. Otherwise, for each scheduled `(vg, env)`, open a sub-session and invoke `run_graph` (sequentially in the v1 implementation; parallelism is a later slice). Each sub-session is `prov:wasInfluencedBy` the top-level aggregation session for chain tracing.
6. Collect all relevant `VerificationGraphResult`s (newly produced + reused).
7. Call [FT-097](FT-097)'s `aggregate_verdict(AggregationTarget::Feature(ft), &results)` — once per TC, then per feature.
8. Render: per-graph table, per-TC verdict table, coverage gap list, aggregate verdict block.
9. Map aggregate verdict → exit code; coverage gap present → exit 3 regardless of verdict (the operator must see the gap before treating an "approved" as final).

#### MCP semantics

- `dec_verify_graph_run` returns `{ session_id, result_id, verdict, step_outcomes, emitted_feedback }`. Streaming via `progress` events keyed by step index.
- `dec_verify_feature` returns `{ session_id, per_graph: [{vg, env, result_id, verdict}], per_tc: [{tc, verdict, rationale, from_results}], coverage_gaps: [...], aggregate: {verdict, rationale} }`.
- Neither MCP tool has a separate "accept" twin — runs are non-destructive on the source graphs and the result artifact write is the side effect, not a separate gesture (contrast with [FT-049](FT-049)'s `generate` / `accept` split, which exists because authoring is Level-3 and needs a review gate).

### Invariants

- Single-handler discipline per [ADR-029](ADR-029): CLI and MCP routes converge.
- The handlers **never re-implement** the per-graph runner; every execution goes through `core::verify::run_graph`. This means `--no-feedback`, `--keep-tmp`, and `--capture` are flag → request-field translations, not branches inside the executor.
- `dec verify feature` is **deterministic in enumeration**: given the same store state, the same set of `(vg, env)` tuples are scheduled. The freshness window is the only time-dependent input.
- Coverage gap is reported as exit 3 even when the per-TC verdicts are all `approved` for covered TCs. An "approved" feature with uncovered TCs is not approved at the feature level — this is the [ADR-031](ADR-031) chain-integrity stance carried into the verb's UX.
- `--dry-run` writes **no** artifacts (no session, no result). The MCP `dry_run: true` form is identical.
- A `Session` artifact is opened **before** the run starts. If the handler crashes mid-run, the session is left as `status = failed` by the harness's existing session-recovery sweep ([FT-022](FT-022)); the operator sees the partial outcome rather than a silent disappearance.

### Error handling

- Unknown `VG-NNN` or `FT-NNN` → `Error::ArtifactNotFound`; exit 1.
- `--environment ENV-NNN` not found → `Error::ArtifactNotFound`; exit 1.
- Graph references env that no longer exists → `Error::OrphanedReference { graph, env }`; exit 1 (the graph needs a `--migrate-env` flow, out of scope here).
- Executor returns `Error::SafetyViolation` → exit 1 with the message; the session is closed `status = failed`. The result artifact (which the executor still writes per [FT-098](FT-098) Phase 1) records the verdict as `rejected` so the run is auditable.
- Streaming format requested over a transport that cannot stream (e.g. `--format sse` over a captured stdout) → fall back to `--format text` with a stderr warning; exit code unchanged.
- `dec verify feature` with no covering graphs at all → exit 3, output names the missing coverage. **No empty success.**

### Boundaries

- **In scope.** The two CLI verbs, their MCP twins, the renderer (text + json + sse), the freshness window, the dry-run enumerator, the session-record wiring, the exit-code mapping, integration tests that exercise both verbs against fixture graphs (success, failure, coverage-gap, unrunnable).
- **Out of scope.** The runner itself ([FT-098](FT-098)). The result artifact shapes and aggregation rule ([FT-097](FT-097)). Auto-dispatch ([FT-100](FT-100)). Parallel execution of multiple graphs in one feature run (sequential v1; a later slice may add a `--parallel <N>` flag). Updating the chain-integrity gate ([FT-047](FT-047)) to call the aggregation function (separate slice). Visualisation of a graph DAG ([ADR-028](ADR-028) §Storage format defers this to slice 3+ in a separate feature).

## Out of scope

- Runner internals.
- Result artifact shapes.
- Auto-dispatch.
- Parallel graph execution.
- Modifying the chain-integrity gate.
- Graph-DAG visual rendering.
- Long-poll / wait-for-result UX (the verbs run synchronously; subscription-driven runs surface through `dec session list`).
- Side-by-side diff between two `VerificationGraphResult`s (a useful debugging verb but its own slice).
