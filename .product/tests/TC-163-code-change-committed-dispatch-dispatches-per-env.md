---
id: TC-163
title: code_change_committed_dispatch dispatches per env and writes an aggregate session with composed verdict
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-100
  adrs: []
phase: 1
---

## Claim

When a `dec:CodeChange` is committed for a feature `FT` (via `dec implement FT` or any path that emits `dec:CodeChangeCommitted`), the `code_change_committed_dispatch` subscription:

1. Enumerates every `VerificationGraph` whose `dec:verifies` or `dec:providesEvidenceFor` chain touches `FT`, filtered by the per-stream env config.
2. Emits one `VerifyGraphRunDispatchEvent` per `(graph, env)` tuple.
3. After all per-graph runs complete, writes one **aggregate session** with role `verify-graph-runner-aggregate`, `status = completed`, `dec:aggregateVerdict` set per FT-097's aggregation function, and `prov:wasInformedBy` the triggering `dec:CodeChangeCommitted` event.
4. If the aggregate verdict is `rejected`, emits one feature-level `dec:Feedback { class: "regression", target: FT }` in addition to the per-step feedback FT-098 already emitted.

## Scenarios

### Setup

- Fresh `.dec/` initialised via `dec init` (registers `code_change_committed_dispatch`).
- Confirm via `dec subscription list` that the subscription is registered with `enabled = true`.
- Seed env `ENV-001 (ephemeral-cli)` and `ENV-002 (ephemeral-cli-alt)` — two ephemeral envs to exercise per-env fan-out.
- Seed feature `FT-CC` with TCs `[TC-CC-A, TC-CC-B]`.
- Seed two graphs:
  - `VG-CC-1` in `ENV-001`, covers TC-CC-A with a passing shell-command.
  - `VG-CC-2` in `ENV-002`, covers TC-CC-B with a passing shell-command.

### Scenario A — happy-path aggregate approved

1. Trigger a `CodeChange` for `FT-CC` (the simplest way: `dec implement FT-CC` against a fixture that the implementer trivially satisfies; alternatively, the test directly emits a `dec:CodeChangeCommitted` event via the event-bus test harness).
2. Wait for the aggregate session to appear (`dec session list --role verify-graph-runner-aggregate`, bounded timeout 60 s).
3. Assertions:
   - Exactly **two** `verify-graph-runner` per-graph sessions exist, one for each `(VG-CC-N, ENV-N)`.
   - Exactly **one** `verify-graph-runner-aggregate` session exists with `prov:wasInformedBy` the triggering CodeChangeCommitted event.
   - The aggregate session's `dec:aggregateVerdict = "approved"`.
   - Two `VerificationGraphResult` artifacts exist on disk.
   - No feature-level `Feedback` artifact is emitted (verdict is approved).

### Scenario B — aggregate rejected emits feature-level feedback

1. Mutate `VG-CC-2`'s step to fail (e.g. `dec verify step add` overwrites `dec:expectExitCode` to a mismatched value).
2. Trigger another CodeChange for `FT-CC`.
3. Wait for the aggregate session.
4. Assertions:
   - Two per-graph sessions, one with verdict `approved` and one with `rejected`.
   - Aggregate session with `dec:aggregateVerdict = "rejected"` (rejection dominates per TC-153).
   - Exactly one `dec:Feedback` artifact with `dec:class = "regression"`, `dec:target = FT-CC`, body referencing the failing TC. This is the feature-level rollup feedback; per-step feedback emitted by the runner itself is separately present (TC-156 asserts that path).

### Scenario C — per-env fan-out independence

Use Scenario A's setup but make `VG-CC-1` fail and leave `VG-CC-2` passing. Assertions:

- Both per-graph runs complete (failure on `ENV-001` does not skip the queued `ENV-002` run).
- Aggregate verdict is `rejected`.
- The order in which the per-graph sessions complete is not asserted (sequential v1 means deterministic order, but the TC asserts both ran, not ordering).

### Scenario D — coverage gap at commit time

1. Seed feature `FT-NOCOV` with TCs but no graphs.
2. Trigger a CodeChange for `FT-NOCOV`.
3. Wait for the aggregate session.
4. Assertions:
   - Zero per-graph sessions (nothing to run).
   - One aggregate session with `dec:aggregateVerdict = "rejected"`, rationale containing `"no covering verification graphs"`.
   - One `dec:Feedback` with `dec:class = "gap"` (not `regression`), `dec:target = FT-NOCOV`.

## Runner

`bash tests/scripts/tc-163-code-change-committed-dispatch.sh`. Same pattern as TC-162: init a temp `.dec/`, start the orchestrator, exercise each scenario, assert the resulting session and artifact state. Triggering a CodeChange in a test may be done via a thin helper (`dec test emit code-change-committed --feature FT-CC --code-change CC-FIXTURE-001`) or by `dec implement` against a trivial fixture; the script's chosen mechanism is documented in its preamble.

## Non-goals

- Auto-amend dispatch on `rejected` aggregate (out of scope; future slice).
- Parallel per-graph execution (sequential v1).
- Cross-stream code-change events (single stream in v1).
- The implementer's behaviour producing the CodeChange (FT-011 / FT-017 cover that).
