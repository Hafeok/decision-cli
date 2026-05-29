---
id: FT-100
title: 'decision-cli: auto-dispatch verify-graph-runner on graph accept and CodeChange commit'
phase: 3
status: in-progress
depends-on:
- FT-097
- FT-098
- FT-099
adrs:
- ADR-028
- ADR-031
tests:
- TC-162
- TC-163
- TC-164
domains: []
domains-acknowledged: {}
---

## Description

Two subscriptions that auto-fire [FT-098](FT-098)'s `core::verify::run_graph` handler at the two natural moments verification should run:

- **`graph_accepted_dispatch`** — fires when a new `dec:VerificationGraph` artifact lands in the store (via [FT-049](FT-049)'s accept path, [FT-041](FT-041)'s manual `dec verify graph new`, or any future writer). Validates the new graph against its declared env in one run; if it fails, the graph itself is suspect (the procedure is broken before the code-under-test ever changes). Mirrors [FT-050](FT-050)'s pattern for verify-graph-author.
- **`code_change_committed_dispatch`** — fires when an implementer's `CodeChange` lands ([FT-017](FT-017)). Enumerates every `VerificationGraph` whose `dec:verifies` (or whose `dec:providesEvidenceFor` chain) touches the feature the implementer targeted, schedules a `run_graph` per `(vg, env)` tuple, and rolls them up through [FT-097](FT-097)'s aggregation rule. This is the **closing of the action-interpretation loop** ([ADR-017](ADR-017), [ADR-019](ADR-019)) that ADR-028 promised: a code change automatically gets a typed interpretation, not just an LLM verdict.

Neither subscription is a CLI surface. They live alongside the existing slice-2 subscriptions (`verifier_dispatch`, `verify_graph_author_dispatch`) under `core::subscriptions::*`. The manual entry verbs are [FT-099](FT-099); this slice triggers the same handlers from events.

One subcommand → one slice — no subcommand; the slice covers the two subscription modules end-to-end (event predicates, dedup ledgers, handler invocation, session bookkeeping).

## Functional Specification

### Inputs

#### `graph_accepted_dispatch`

- An event from oxi-events ([ADR-001](ADR-001)) whose payload type is `dec:VerificationGraphCreated` (emitted by the `StreamWriter` chokepoint on every successful write of a `dec:VerificationGraph` artifact, regardless of source).
- Per-stream config under `.dec/config.toml`:
  ```toml
  [verify_graph_runner.on_graph_accepted]
  enabled = true                       # default true
  envs = ["*"]                         # restrict to a subset of env IRIs; "*" = the graph's declared env
  dedup_ttl_seconds = 300              # 5 min dedup window — re-edits within window are coalesced
  ```

#### `code_change_committed_dispatch`

- An event of type `dec:CodeChangeCommitted` (already emitted by [FT-017](FT-017) when the implementer finalises).
- The associated `Feature` and `CodeChange` IRIs (carried on the event).
- Per-stream config:
  ```toml
  [verify_graph_runner.on_code_change]
  enabled = true                       # default true
  envs = ["*"]                         # which envs to verify across; "*" = every env with a covering graph
  parallelism = 1                      # sequential v1 (matches FT-099 semantics)
  fan_out = "per_env"                  # per_env | per_graph — controls one-session-per-graph vs one-session-per-env
  dedup_ttl_seconds = 60               # short window since CodeChange events are rare and meaningful
  ```

### Outputs

- One `VerifyGraphRunDispatchEvent` per scheduled `(graph, env)` tuple — same event-shape pattern as `VerifierDispatchEvent` / `VerifyGraphAuthorDispatchEvent`. The orchestrator picks these up and dispatches them through the same `core::verify::run_graph` handler [FT-099](FT-099) drives.
- One `dec:Session` artifact per dispatch with role `verify-graph-runner`, `status = running` → `completed`/`failed`, `prov:wasInformedBy` the triggering event.
- The runner's standard side effects: a `VerificationGraphResult` per `(graph, env)`, `Feedback` artifacts on failure (or `code_change_committed_dispatch` may set the per-stream `feedback_routes` to skip emission — see Behaviour §3).

### State

- New subscriptions registered at `dec init` alongside `verifier_dispatch` and `verify_graph_author_dispatch`.
- Two new dedup ledgers in the store, one per subscription, keyed on:
  - `graph_accepted_dispatch`: `(graph_iri, env_iri)` → `last_dispatched_at`.
  - `code_change_committed_dispatch`: `(code_change_iri, feature_iri)` → `last_dispatched_at`.
- One additional session-status path: a `verify-graph-runner` session is recognised by `dec session list`. No new schema — reuses the existing session vocabulary from slice 2.

### Behaviour

#### `graph_accepted_dispatch`

1. Receive `dec:VerificationGraphCreated { graph_iri, env_iri }`.
2. Check `enabled` — if false, no-op.
3. Resolve the env set: `envs = ["*"]` means the graph's declared env only (`env_iri`); otherwise the configured subset, filtered to envs the graph references via its steps (e.g. an `http-request` step's endpoint env).
4. For each env, consult the dedup ledger. Within the TTL → no-op (coalesces rapid re-edits during authoring).
5. Otherwise, emit one `VerifyGraphRunDispatchEvent { graph: graph_iri, env: env_iri, trigger: GraphAccepted, run_activity: <new IRI> }`.
6. Update the ledger entry to now.
7. The orchestrator dispatches; the handler invokes `core::verify::run_graph` with `triggered_by = TriggerKind::GraphAccepted`. No special bundle: the graph is the authoritative input.

#### `code_change_committed_dispatch`

1. Receive `dec:CodeChangeCommitted { code_change_iri, feature_iri, committed_at }`.
2. Check `enabled` — if false, no-op.
3. Enumerate covering graphs for the feature: same SPARQL as [FT-099](FT-099)'s `dec verify feature` step 2, filtered to the configured env subset.
4. **Chain-integrity intersection.** If `[FT-047](FT-047)`'s gate already blocked the implementer dispatch on uncovered TCs, the commit event would not have fired in the first place — so by the time this subscription runs, coverage exists by construction (or was waived). The subscription still re-checks coverage to defend against the edge case where coverage was deleted between dispatch and commit; missing coverage at this point emits a single `dec:CoverageGap` feedback against the feature (not against any TC) and proceeds with the runs it *can* schedule.
5. For each `(graph, env)` tuple:
   - Consult the dedup ledger: skip if within TTL (rare — CodeChange events are not bursty, but a hand-triggered re-commit within 60 s should not double-run).
   - Pre-bind `${code_change_path}` to the commit's worktree path (if `repo-path` env) or `${code_change_ref}` to the commit SHA (if `remote-http` env testing a deployed build).
   - Emit a `VerifyGraphRunDispatchEvent { graph, env, trigger: CodeChangeCommitted { code_change }, capture_bindings: {...}, run_activity }`.
6. After all per-graph dispatches complete (sequentially in v1; the subscription waits on the session-completion events before composing), invoke [FT-097](FT-097)'s `aggregate_verdict` over the collected results and write an **aggregate result session** with role `verify-graph-runner-aggregate`, `status = completed`, `dec:aggregateVerdict = <verdict>`. This is the artifact the dashboard / chain-integrity gate / fitness functions read for "is this CodeChange interpretable as satisfying its feature?"
7. If the aggregate verdict is `rejected` and `feedback_routes != "suppress"`, emit one `dec:Feedback { class: "regression", target: feature_iri }` summarising which TCs failed. This is in addition to the per-step feedback the runner already emitted in [FT-098](FT-098); the aggregate feedback is the **feature-level** rollup that the routing subscription ([FT-029](FT-029)) sends back to the implementer role for an amend cycle.

#### Idempotency and replay

- Both subscriptions read their dedup ledger from the store before dispatching, and write back on success — replaying events from a sequence number does not double-dispatch within the TTL window.
- An aggregate session has a deterministic IRI keyed on `(code_change_iri, feature_iri, batch_start_time_truncated_to_second)` so replay of the CodeChange event produces the same aggregate session IRI if and only if the per-graph runs would land on the same VGR IRIs. Differences in clock or scheduling produce a new aggregate session; the operator can distinguish via `prov:wasInformedBy`.

### Invariants

- The subscriptions **never persist VerificationGraphResults or Feedback directly** — they emit dispatch events; the runner ([FT-098](FT-098)) writes the artifacts. The subscriptions only write the dedup ledger entries and session records.
- A `code_change_committed_dispatch` aggregate verdict is **the** signal for the action-interpretation pairing ([ADR-017](ADR-017)). The `DispatchGroup` ([FT-021](FT-021)) pairs the implementer's `CodeChange` to this aggregate result; both feed into the agreement metric ([FT-024](FT-024)) once the runner-vs-verifier comparison lands in a later slice.
- A `code_change_committed_dispatch` that finds **zero covering graphs** does not silently pass. It writes a `dec:CoverageGap` feedback and produces an aggregate session with `verdict = rejected`, rationale `"no covering verification graphs for feature; chain-integrity gate likely waived"`. The implementer's dispatch can still complete (the gate already let it through), but the feature is marked rejected at the verification layer until graphs land.
- Per-env dispatch is **independent** — a failing run against `ENV-002 (ephemeral-cli)` does not skip the queued run against `ENV-003 (dev-deployment)`. The aggregate verdict is computed from the full result set, not aborted on first fail.
- Both subscriptions are **registered at `dec init`** and recorded in the store as `dec:Subscription` artifacts (same convention as the slice-2 dispatch subscriptions). `dec subscription list` lists them; `dec subscription disable <id>` flips `enabled = false` per-stream without code changes.
- Sessions opened by the subscriptions carry `prov:wasInformedBy = <triggering_event>` so the audit chain is complete from event → session → result → aggregate-result.

### Error handling

- A scheduled `run_graph` returns `Error::ArtifactNotFound` (graph deleted between event and dispatch) → log, skip that tuple, continue with others; the aggregate session records the missing graph in `dec:partialFailureReasons`.
- `Error::SafetyViolation` from `run_graph` → the runner has already written a `rejected` result; the aggregate session includes it as a contributing result. The aggregate verdict will be `rejected` per [FT-097](FT-097).
- `Error::ResultWriteFailed` → the dispatch is left as `status = failed`; the operator triages via `dec session show`. The aggregate session waits up to the per-stream `aggregate_timeout_seconds` (default 1800) for missing sub-sessions before composing what it has and marking itself `verdict = amendment-required` with rationale naming the timed-out runs.
- Subscription handler crash → recovered by the existing event-replay logic ([FT-005](FT-005)); the ledger entry written before the crash prevents an immediate re-dispatch storm, and the operator can clear the ledger entry via a maintenance verb (out of scope; manual SPARQL UPDATE in v1).

### Boundaries

- **In scope.** The two subscription modules (`core::subscriptions::graph_accepted_dispatch` and `core::subscriptions::code_change_committed_dispatch`), their event-payload types, dedup ledger plumbing, aggregate-session writing, the `dec:VerifyGraphRunDispatchEvent` type, registration at `dec init`, integration tests that fire each subscription against a fixture event stream and assert the expected dispatch / aggregate outcomes.
- **Out of scope.** The runner itself ([FT-098](FT-098)). The CLI verbs ([FT-099](FT-099)). The chain-integrity gate ([FT-047](FT-047)) consuming aggregate verdicts as its gating signal (separate slice — for now the gate still consumes the coverage primitive). The dashboard / metrics consumers of the aggregate session. Parallel execution across envs (sequential v1). Auto-amend dispatch on `rejected` aggregate (future slice closes the loop end-to-end; for now the feedback is emitted and surfaces in `dec feedback list`).

## Out of scope

- Runner internals.
- CLI / MCP entry.
- Chain-integrity gate changes.
- Auto-amend dispatch on rejected aggregate.
- Parallel cross-env execution.
- Dashboard / metrics consumption.
- Re-firing on TC-modification events directly (the subscriptions react to graph-create and code-change-commit; a TC body edit is not a verification-relevant event by itself).
- Backpressure / queue management beyond the dedup TTL (a future slice may add priority queues if subscription event volume grows).
