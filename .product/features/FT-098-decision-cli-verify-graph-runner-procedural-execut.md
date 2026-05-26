---
id: FT-098
title: 'decision-cli: verify-graph-runner — procedural executor for VerificationGraph'
phase: 3
status: planned
depends-on:
- FT-097
adrs:
- ADR-028
- ADR-031
tests:
- TC-154
- TC-155
- TC-156
- TC-157
domains: []
domains-acknowledged: {}
---

## Description

The procedural executor that turns a `dec:VerificationGraph` into a `dec:VerificationGraphResult` ([FT-097](FT-097)). Walks the graph's ordered steps against its declared `dec:VerificationEnvironment`, runs each step according to its typed kind, captures a per-step trace, and on completion writes the result artifact through the existing `StreamWriter` chokepoint ([FT-001](FT-001)). When a step fails or is unrunnable, emits a `dec:Feedback` artifact ([FT-026](FT-026)) per the FT-031 SDK so the routing subscription ([FT-029](FT-029)) can surface it to the responsible upstream role.

**This is the slice-3 graph executor [ADR-028](ADR-028) §Relationship-to-FT-023's-worker promised, and the partner [ADR-031](ADR-031) anticipated.** It is the second role implemented in-process rather than as a worker subprocess (the first being the dispatcher itself). The motivation is the same one [ADR-028](ADR-028) records: verification becomes "execute the graph, capture per-step trace, aggregate per-step outcomes, emit verdict" — no LLM call on the hot path. Subprocess overhead and a worker contract would be ceremony around a procedural state machine.

This slice implements the **six seed step kinds** from [ADR-028](ADR-028)'s vocabulary: `shell-command`, `sparql-assertion`, `file-assertion`, `http-request`, `wait-for`, `capture`. Later slices extend the kind registry (e.g. `llm-judgment`, `dagger-pipeline`); each extension is its own feature that adds a kind handler — this slice ships the framework that admits them.

One subcommand → one slice — there is no subcommand here; the slice is the **executor function and its kind dispatch table**. The CLI/MCP entry that calls it is [FT-099](FT-099); the subscription that auto-invokes it is [FT-100](FT-100). Both consumers ultimately call the single `core::verify::runner::run_graph` handler defined in this slice.

## Layout — where this slice lives in the source tree

This feature **extends `core` directly**; it does *not* land in a `features/ft_098_*/` directory. Per [CLAUDE.md §Discipline within decision-cli](../../CLAUDE.md) — *"When patterns recur across features, migrate to core. Vertical slice tolerates some redundancy in exchange for slice independence. When two slices grow similar code, that pattern is a candidate for `core` — author a feature_spec for the core extension, then individual slices adopt it."* Both [FT-099](FT-099) (the CLI/MCP entry) and [FT-100](FT-100) (the subscription) call `run_graph`, so by the rule the executor belongs in `core` from day one — this feature_spec **is** the "feature_spec for the core extension" the principle requires.

The runner sits next to the existing slice-2.5 verify substrate (`core/verify/chain_integrity/`, `coverage/`, `matcher/`, `safety.rs`):

```
crates/decision-cli/src/core/verify/
    chain_integrity/         (existing — FT-047 / ADR-031)
    coverage/                (existing — FT-045)
    matcher/                 (existing — FT-046)
    safety.rs                (existing — FT-037, authoring-time op gate)
    aggregate.rs             (FT-097 — pure aggregate_verdict() function)
    runner/                  (THIS SLICE)
        mod.rs               — pub fn run_graph(req: RunGraphRequest)
                                  -> Result<RunGraphResponse, RunnerError>;
                                  pub trait StepKindHandler;
                                  KindDispatchTable (registers the six seed handlers)
        request.rs           — RunGraphRequest / RunGraphResponse / TriggerKind types
        context.rs           — RunContext, capture-binding table, ${name} substitution
        env_lifecycle.rs     — Phase 2 (setup, workdir resolution) + Phase 4 (teardown)
        runtime_safety.rs    — Phase 1 defensive re-check (delegates to safety.rs)
        trace_writer.rs      — builds VerificationGraphResult, writes via StreamWriter
        feedback.rs          — emits dec:Feedback for failed evidence-bearing steps
        kinds/
            mod.rs           — re-exports the six kinds; registry is built here
            shell.rs         — StepKindHandler for "shell-command"
            sparql.rs        — for "sparql-assertion" (file + http endpoint targets)
            file.rs          — for "file-assertion"
            http.rs          — for "http-request"
            wait_for.rs      — for "wait-for" (wraps another kind handler)
            capture.rs       — for "capture"
        tests/               — kind-level unit tests + Phase 1..5 integration tests
            shell_tests.rs
            sparql_tests.rs
            ...
```

The new artifact types and the aggregation function from [FT-097](FT-097) land separately:

```
crates/decision-cli/src/core/ontology/
    verification_result.rs   (FT-097 — VerificationStepTrace + VerificationGraphResult Rust types)

crates/decision-cli/src/core/bundled/shapes/
    verification_result.shacl.ttl  (FT-097 — SHACL shapes for the new artifact types)
```

`core/verify/runner/` has **no** `pub` items beyond `run_graph`, `RunGraphRequest`, `RunGraphResponse`, `RunnerError`, and the `StepKindHandler` trait + the registry constructor. Everything else is `pub(crate)` or `pub(super)` so the surface stays narrow and the feature slices that consume it cannot reach inside.

**Public consumers:**

- [FT-099](FT-099) — `features/verify_graph_run/internal.rs` calls `core::verify::runner::run_graph` from its handler. The CLI surface module adapts `clap` args + MCP input into `RunGraphRequest` and renders the `RunGraphResponse`.
- [FT-100](FT-100) — `core/subscriptions/code_change_committed_dispatch/` and `core/subscriptions/graph_accepted_dispatch/` emit `VerifyGraphRunDispatchEvent`s; the orchestrator's existing dispatch loop translates these into `core::verify::runner::run_graph` calls and writes the resulting `Session`.

Both routes converge on **one** `run_graph` function — that's the [ADR-029](ADR-029) single-handler discipline carried into a non-CLI/MCP context.

## Functional Specification

### Inputs

```rust
pub struct RunGraphRequest {
    pub graph: Iri,                          // VG-NNN
    pub triggered_by: TriggerKind,           // CodeChange commit, manual CLI, accept, etc.
    pub capture_bindings: HashMap<String, String>, // pre-seeded captures from the trigger (optional)
    pub run_activity: Iri,                   // PROV-O activity opened by the caller
}

pub enum TriggerKind {
    Manual,                                  // dec verify graph run
    GraphAccepted,                           // FT-100 subscription on accept
    CodeChangeCommitted { code_change: Iri },// FT-100 subscription on commit
    Aggregate { feature: Iri },              // dec verify feature roll-up entry
}
```

- The graph IRI must resolve to a persisted `dec:VerificationGraph`; the executor loads it (and its environment) from the store. **No raw step inputs** — the executor is bundle-agnostic in the sense that *graphs* are the authoritative input, not free-form step lists.
- `capture_bindings` is the slice-3 hook ADR-028 reserved (`${name}` syntax) so a CodeChange-triggered run can pre-bind, e.g., `${code_change_path}` to the implementer's working tree. The executor performs substitution on `dec:command`, `dec:query`, `dec:target`, and `dec:url` strings before invoking each kind handler.
- `run_activity` is created by the caller (CLI handler / subscription) so this slice does not own session creation. The executor returns the result artifact; the caller closes the activity.

### Outputs

- A persisted `dec:VerificationGraphResult` ([FT-097](FT-097)) at `.dec/verify/result/VGR-NNN.ttl`.
- A `RunGraphResponse` returned to the caller:
  ```rust
  pub struct RunGraphResponse {
      pub result: Iri,                     // VGR-NNN
      pub verdict: Verdict,                // per-graph verdict (FT-097's single-graph derivation)
      pub step_outcomes: Vec<StepOutcome>, // ordered, in-memory mirror for the caller's renderer
      pub emitted_feedback: Vec<Iri>,      // FB-NNN artifacts written for failures, if any
  }
  ```
- For every step with `outcome ∈ {fail, unrunnable}` whose parent step declares one or more `dec:providesEvidenceFor` TCs: one `dec:Feedback` artifact written via the existing FT-031 emit-feedback SDK with:
  - `dec:class = "gap"` if `outcome = unrunnable`, `dec:class = "regression"` if `outcome = fail`.
  - `dec:target` set to **each** linked TC (one feedback per TC; the SDK already handles fan-out).
  - `dec:fromActivity = run_activity`.
  - Body: the trace excerpt + the step's expected-vs-actual one-liner.

### State

- Reads: the existing graph (`VG-NNN.ttl` projection in the store), the environment (`ENV-NNN`), TCs referenced by `dec:providesEvidenceFor` (for feedback fan-out).
- Writes: one `VGR-NNN.ttl` per run, one or more `FB-NNN.md` per failed evidence-bearing step. Both go through `StreamWriter` — SHACL-validated, content-hashed, attributed to the runner agent.
- Per-step environment side effects depend on the step kind (a `shell-command` step may create files in `dec:envType = ephemeral-tempdir`; an `http-request` step may produce a server-side mutation, gated by env's `allowedOps`). The executor does not own env setup/teardown — see the env lifecycle section.

### Behaviour

#### Phase 1 — load and pre-flight

1. Load the `VerificationGraph` and its `VerificationEnvironment` from the store. Missing → `Error::ArtifactNotFound`.
2. Re-validate `step.requiredOps ⊆ env.allowedOps` for every step. This is the **defensive replay** [ADR-028](ADR-028) §Safety gating mandates: an env mutated since authoring, or a graph mutated since authoring, must still pass the static check at run time. A violation here aborts the run with `Error::SafetyViolation { step, op }` and writes a `VerificationGraphResult` with `verdict = rejected`, rationale `"safety: step <S> requires op <O> not in env.allowedOps"`, and no step traces (the run never started). Implementation lives in `runner/runtime_safety.rs` and delegates the predicate to the existing `core::verify::safety` module (single source of truth for the op-subset check).
3. Compose initial capture bindings: any `capture_bindings` from the request, merged with kind-default bindings (e.g. `${dec_workdir}` resolved from `dec:envType` once the env's `setup` has run).

#### Phase 2 — env setup

1. If `env.dec:setup` is present, run it as a `shell-command` step with `expectExitCode = 0`. A failing setup short-circuits the run: `verdict = unrunnable` overall (no graph-verdict ambiguity — setup failure is not the graph's fault).
2. Establish the runtime working directory:
   - `envType = "ephemeral-tempdir"` → mktemp under `$DEC_TMP` (default `$TMPDIR/dec-verify`); cleanup is enqueued for Phase 4.
   - `envType = "repo-path"` (per [FT-053](FT-053)) → resolve to the repo-relative path; **no cleanup**.
   - `envType = "remote-http"` → no working directory; HTTP-only kinds.
3. Bind `${dec_workdir}` to the resolved path.

#### Phase 3 — step loop

For each step in `graph.dec:steps` order:

1. **Substitute captures.** Resolve `${name}` references in the step's body fields (`command`, `query`, `target`, `url`, `body`) against the current binding table. A reference to an unbound name → step `outcome = unrunnable`, `errorMessage = "unbound capture: ${name}"`, **continue to the next step** (the trace is captured; the run does not abort).
2. **Dispatch by kind.** Each kind has a handler with a uniform signature:
   ```rust
   trait StepKindHandler {
       fn run(&self, step: &VerificationStep, ctx: &mut RunContext) -> StepTrace;
   }
   ```
   The dispatch table is built in `runner/kinds/mod.rs` and registers the six seed handlers (`shell.rs`, `sparql.rs`, `file.rs`, `http.rs`, `wait_for.rs`, `capture.rs`). Adding a new kind in a later slice = adding one handler module + one registry entry.
   - `shell-command` — spawn `bash -c <command>` in `dec_workdir` with a per-step timeout (default 60 s, overridable by `dec:timeout`). Capture stdout/stderr (4 KiB cap, full payload to sibling log file). `pass` iff exit code matches `dec:expectExitCode`. Output is bound to `${step_N_stdout}` automatically for downstream `capture` steps.
   - `sparql-assertion` — load the `dec:target` (path to a Turtle/N-Quads file under `dec_workdir`, or an `http(s)://` SPARQL endpoint if `envType = remote-http`). Execute the query via oxigraph (in-memory for file targets, HTTP for endpoint targets). `pass` iff the row count matches `dec:expectRows` *or* every triple in `dec:expectTriples` (Turtle fragment) appears in the result. Mismatch → `fail`. Parse error / target missing → `unrunnable`.
   - `file-assertion` — assert existence, content equality (against an inline `dec:expectContent` literal), or SHA-256 hash (`dec:expectSha256`) on the `dec:target` path. Pass/fail/unrunnable as expected.
   - `http-request` — perform the request via `reqwest`; `pass` iff status matches `dec:expectStatus` and (when present) the response body matches `dec:expectBody` (literal) or `dec:expectJsonpath` (subset match). Network failure → `unrunnable`; assertion mismatch → `fail`. Verb gated by `env.allowedOps` (`http` vs `http-mutating`); a verb outside allowedOps at run time → `unrunnable` (defense in depth — the static gate should have caught this).
   - `wait-for` — re-execute a wrapped step until it passes or `dec:timeout` elapses. Polling interval defaults to 1 s, overridable by `dec:pollInterval`. The final wrapped-step trace is recorded as the `wait-for` step's trace; the wait time is folded into `dec:endedAt - dec:startedAt`.
   - `capture` — bind the named target (`dec:bindName = "foo"`) to the value of `dec:source` (one of `prior_step_stdout`, `prior_step_exit_code`, `literal`, `env_var`). Always `pass` (a capture cannot fail; an unbound source is a SHACL violation caught at authoring). The binding is added to the run context for subsequent steps.
3. **Stop conditions.** By default the executor runs **every** step, even after a failure — the failure report should be complete, not partial. Two exceptions:
   - Phase 1 / Phase 2 failures abort before Phase 3 starts.
   - A `shell-command` step with `dec:stopOnFail = true` (optional, default false) ends the loop early; remaining steps record `outcome = unrunnable` with `errorMessage = "skipped: prior step <N> halted the run"`.
4. **Append trace.** Each step produces one `VerificationStepTrace`; the executor maintains an ordered vector mirroring `graph.dec:steps`.

#### Phase 4 — env teardown

1. If `env.dec:teardown` is present, run it as a `shell-command` step (recorded as the *teardown* trace under a sibling activity, not part of `stepTraces`).
2. For `envType = ephemeral-tempdir`, remove the temp directory unless `DEC_KEEP_TMP=1` is set (debug only).
3. Teardown failures are logged but do **not** alter the graph verdict — teardown is hygienic, not semantic.

#### Phase 5 — verdict derivation and persistence

1. Derive the per-graph verdict using [FT-097](FT-097)'s single-graph rule.
2. Materialise the `evidenceFor` projection: for each step with `dec:providesEvidenceFor`, for each linked TC, emit one `EvidenceProjection { tc, outcome, fromStep }`.
3. Build the rationale string. Templates:
   - All passed: `"all <N> steps passed; <M> TCs received pass evidence"`.
   - Some failed: `"step <N> (<kind>) failed: <errorMessage>; <M> TCs affected"`.
   - Setup failure: `"env setup failed before any step ran: <errorMessage>"`.
   - Safety abort: `"safety: step <S> requires op <O> not in env.allowedOps"`.
4. Persist `VGR-NNN.ttl` through `StreamWriter` (`runner/trace_writer.rs`).
5. For each `(step, TC)` pair whose step is `fail` or `unrunnable`, emit a `Feedback` artifact (`runner/feedback.rs`). Feedback writes are best-effort — a feedback-write failure is logged and does **not** alter the result verdict (the result artifact is the contract; feedback is a downstream convenience).
6. Return `RunGraphResponse` to the caller.

#### Idempotency and concurrency

- The executor is **single-flight per `(graph, env, run_activity)` tuple** — the caller is responsible for ensuring it does not launch two concurrent runs against the same graph with the same activity. (Different activities running the same graph in parallel is fine; that's the multi-trigger case.)
- Repeated invocations produce a new `VGR-NNN` each time. There is no in-place update; the chain-integrity gate consumes latest-by-`dcterms:created` per `(graph, env)`.

### Invariants

- The executor **never bypasses** `StreamWriter` for any write. SHACL enforcement at write time is the chokepoint that gives [FT-097](FT-097)'s invariants their teeth.
- The executor **never calls an LLM** on its own. The framework reserves `llm-judgment` as a future kind that *would* dispatch to a worker, but that handler is out of scope here. As of this slice, every run is purely procedural.
- The executor **never modifies** the input `VerificationGraph` or `VerificationEnvironment`. They are read-only inputs.
- `dec:stepTraces` length on the persisted result equals `graph.dec:steps` length **exactly**. Steps that are skipped after `stopOnFail` still have a trace entry (with `outcome = unrunnable`). This is the SHACL contract [FT-097](FT-097) enforces; the runner upholds it by construction.
- Capture bindings live only for the duration of one run. They are not persisted, not exposed cross-run, not visible in the result artifact (their effects show up in the substituted command/query text recorded per-step).
- Run-time op enforcement is **independent** of authoring-time enforcement ([FT-037](FT-037)) — both run. A mutation between authoring and execution must be caught by either, never by neither. `runner/runtime_safety.rs` and `core::verify::safety` share the same predicate function so the two gates can never drift.
- `core/verify/runner/` exposes a **narrow public surface** — only `run_graph`, the request/response/error types, and the `StepKindHandler` trait + registry constructor are `pub`. Internal modules (`context`, `env_lifecycle`, `trace_writer`, `feedback`, the individual kind handlers) are `pub(crate)` at most. Feature slices that consume the runner cannot reach inside it.

### Error handling

- `Error::ArtifactNotFound { iri }` — graph or env missing.
- `Error::SafetyViolation { step, op }` — Phase 1 op-gate fail (also emits a result with `verdict = rejected`).
- `Error::EnvSetupFailed { exit_code, stderr_excerpt }` — Phase 2 setup script failed (emits a result with `verdict = amendment-required`).
- `Error::ResultWriteFailed { source }` — `StreamWriter` rejected the result (SHACL violation, content-hash collision, etc.). The activity is left open for the caller to close with a failure note; no result is persisted.
- Per-step errors do **not** propagate as `Error::*` — they are encoded as `outcome = fail / unrunnable` on the trace. The executor only returns `Result::Err` for failures that prevent producing a result artifact at all.

### Boundaries

- **In scope.** The `core::verify::runner::run_graph` handler (and the supporting modules under `core/verify/runner/` listed in §Layout above), the kind-handler trait + registry, the six seed-kind implementations, capture substitution, env setup/teardown, result persistence via `StreamWriter`, feedback emission on failure, defensive run-time op check, kind-handler unit tests, and one integration test per seed kind (run the kind through `run_graph` against a fixture graph in an ephemeral env, assert the persisted result).
- **Out of scope.** Manual CLI / MCP entry — that is [FT-099](FT-099); this slice exposes only the handler. Subscription wiring — that is [FT-100](FT-100). Aggregate verdict computation across multiple results — that is [FT-097](FT-097)'s pure function (called by [FT-099](FT-099)'s `dec verify feature` and by the chain-integrity gate, not by the runner). `llm-judgment` kind. `dagger-pipeline` kind. Full-payload log persistence beyond excerpt (reserved predicates exist; a later slice fills them). Retry / backoff on transient failures — `unrunnable` is recorded once and surfaced; the operator re-triggers via [FT-099](FT-099). Updating the chain-integrity gate ([FT-047](FT-047)) to consume results (separate slice).

## Out of scope

- CLI / MCP surface.
- Subscription wiring.
- LLM-judgment kind.
- Pipeline (Dagger / Buildkite / etc.) kind.
- Multi-graph aggregation (lives in [FT-097](FT-097)).
- Full-payload log files (excerpt-only).
- Transient-failure retry.
- Modifying the chain-integrity gate.
- Cross-host or remote execution (this slice runs every kind locally; remote endpoints in `http-request` are HTTP-call destinations, not remote executors).
- A `features/ft_098_*/` directory — this slice extends `core` directly per the rationale in §Layout above.
