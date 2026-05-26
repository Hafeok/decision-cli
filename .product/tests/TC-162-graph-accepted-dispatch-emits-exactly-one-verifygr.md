---
id: TC-162
title: graph_accepted_dispatch emits exactly one VerifyGraphRunDispatchEvent per accepted graph and opens one Session
type: scenario
status: passing
validates:
  features:
  - FT-100
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-162-graph-accepted-dispatch.sh
runner-timeout: 180
last-run: 2026-05-26T14:37:12.345693727+00:00
last-run-duration: 0.6s
---

## Claim

When a new `dec:VerificationGraph` artifact lands in the store (via `dec verify graph new`, `dec verify graph generate --accept`, or any direct `StreamWriter::write`), the `graph_accepted_dispatch` subscription emits exactly one `VerifyGraphRunDispatchEvent` for that `(graph, env)` tuple, the orchestrator picks it up and invokes `core::verify::runner::run_graph`, and exactly one `Session` artifact with role `verify-graph-runner` is created with `prov:wasInformedBy` the triggering `dec:VerificationGraphCreated` event.

## Scenarios

### Setup

- Fresh `.dec/` initialised via `dec init` (registers the new subscription).
- Confirm via `dec subscription list` that `graph_accepted_dispatch` is registered and `enabled = true`.
- Seed env `ENV-001 (ephemeral-cli)`.

### Scenario A — single graph, single env (the happy path)

1. Snapshot the current `dec session list` count for role `verify-graph-runner` (expected 0).
2. Snapshot the current `.dec/verify/result/*.ttl` count (expected 0).
3. Author a new graph via `dec verify graph new VG-NEW --verifies FT-FIXTURE --environment ENV-001` plus one `dec verify step add` for a trivial passing shell-command.
4. **Wait for the subscription to fire** (the test polls `dec session list --role verify-graph-runner` with a bounded timeout, e.g. 30 s). Polling cadence: 500 ms.
5. Assertions once a session appears (or timeout reached):
   - Exactly one new session exists with role `verify-graph-runner`.
   - That session's `prov:wasInformedBy` points at the `dec:VerificationGraphCreated` event whose payload includes `graph = VG-NEW`.
   - Exactly one `VerificationGraphResult` artifact exists at `.dec/verify/result/VGR-N.ttl` with `dec:resultOf = VG-NEW` and `dec:ranInEnvironment = ENV-001`.
   - The session's status is `completed` (the trivial graph passes).

### Scenario B — config `enabled = false` suppresses the dispatch

1. Edit `.dec/config.toml` to set `[verify_graph_runner.on_graph_accepted].enabled = false`.
2. Snapshot session count.
3. Author another graph `VG-DISABLED` identical in shape to `VG-NEW`.
4. Wait the same bounded timeout. Assertion: no new `verify-graph-runner` session appears; no new `VerificationGraphResult` is written.

### Scenario C — dedup ledger entry is written and read

1. Re-enable the subscription (config `enabled = true`).
2. Author `VG-DEDUP` and let the subscription fire (Scenario A path).
3. Assert via SPARQL against the in-store dedup ledger that an entry exists with `(graph = VG-DEDUP, env = ENV-001, last_dispatched_at = <recent timestamp>)`.
4. Re-trigger the same `dec:VerificationGraphCreated` event (via direct event-replay in the test, simulating an `oxi-events` replay from a sequence number). Within the dedup TTL window (default 300 s), the subscription must **not** dispatch a second run.

### Scenario D — graph references env not in the catalog

Author a graph that references `ENV-DOES-NOT-EXIST`. Expectation: the subscription logs an error and does not dispatch (a missing env cannot be verified against). No session is created; no result artifact. Stderr of the subscription contains a structured log entry naming the missing env.

## Runner

`bash tests/scripts/tc-162-graph-accepted-dispatch.sh`. The script must:

1. Init a temp `.dec/` and start the orchestrator background process (`dec orchestrator start` or whatever the slice-2 verb is) so the subscription has an event loop to fire on.
2. Run Scenarios A–D in sequence, asserting the documented outcomes.
3. Stop the orchestrator and tear down the temp directory on exit.

The bounded poll in Scenarios A/C/D uses a 30 s upper bound; if the subscription has not fired by then, the test fails with a diagnostic (likely a subscription registration issue or an event-bus stall).

## Non-goals

- The runner's behaviour during the dispatched run (FT-098 TCs cover that — this TC asserts the *dispatch happened*, not what the runner did with it).
- Cross-stream subscription behaviour (single stream in v1).
- The orchestrator's event-loop implementation (covered by oxi-events TCs).
- Subscription `--restart` recovery behaviour beyond Scenario C's replay assertion.