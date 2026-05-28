---
id: TC-164
title: subscription dedup TTL coalesces rapid re-fires within the window into a single dispatch
type: scenario
status: failing
validates:
  features:
  - FT-100
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-164-subscription-dedup.sh
runner-timeout: 180
last-run: 2026-05-28T08:48:49.733702120+00:00
last-run-duration: 1.4s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

Both new subscriptions (`graph_accepted_dispatch` and `code_change_committed_dispatch`) maintain a per-`(key, env)` dedup ledger in the store and **do not** emit a second dispatch event when the same key fires again within the configured TTL — even if the same triggering event is replayed from oxi-events (FT-005). After the TTL has elapsed, the next fire produces a new dispatch.

## Scenarios

### Setup

- Fresh `.dec/` with both subscriptions registered.
- Configure `.dec/config.toml` with a short TTL for testability:
  ```toml
  [verify_graph_runner.on_graph_accepted]
  dedup_ttl_seconds = 5

  [verify_graph_runner.on_code_change]
  dedup_ttl_seconds = 5
  ```
- Seed env `ENV-001`, feature `FT-DEDUP`, graph `VG-DEDUP-1` covering one TC.

### Scenario A — `graph_accepted` dedup window

1. Snapshot session count for role `verify-graph-runner`.
2. Author `VG-DEDUP-1` and wait for the subscription to fire (one new session expected).
3. **Within 2 s** (well inside the 5 s TTL), simulate a `dec:VerificationGraphCreated` event replay for `VG-DEDUP-1` by calling the oxi-events replay helper (`dec events replay --from <seq>` or the test-only equivalent that re-injects the captured event).
4. Wait 3 s (still inside the TTL). Assertion: session count for `verify-graph-runner` has **not** increased — the replay was coalesced.
5. Wait until total elapsed time exceeds the TTL (i.e. wait an additional 5 s past the original fire).
6. Replay the same event one more time. Assertion: session count has **increased by exactly 1** — the TTL window expired and the subscription dispatched again.

### Scenario B — `code_change_committed` dedup window

1. Trigger a `CodeChange` for `FT-DEDUP` (any path that produces the event).
2. Wait for the aggregate session to appear. Snapshot the count.
3. **Within 2 s**, trigger the same `CodeChange` again (same `code_change_iri` — the test helper allows this; in production this would be a replay event, not a new implementer run).
4. Wait 3 s. Assertion: no new aggregate session created; no new per-graph sessions for the dedup-coalesced second event.
5. Wait past the TTL, trigger again. Assertion: a new aggregate session is created.

### Scenario C — dedup is per-`(key, env)`, not global

Using Scenario A's setup, immediately after the first `VG-DEDUP-1` dispatch (within the TTL), author a **second** graph `VG-DEDUP-2` (different key). Assertion: the second author dispatches *immediately* (dedup window is per `(graph, env)`, not a global rate limit) — session count increments by 1 promptly.

### Scenario D — ledger entries are persisted and survive restart

1. Trigger a dispatch in Scenario A's setup (let the ledger entry be written).
2. Stop the orchestrator process.
3. Restart it.
4. Within the original TTL window (measured from the first dispatch), replay the triggering event.
5. Assertion: no dispatch — the persisted ledger entry survived restart, and the subscription's dedup check honours it.

## Runner

`bash tests/scripts/tc-164-subscription-dedup.sh`. The script needs `sleep`/`date` math for the wait windows; on machines under heavy load the 2 s / 3 s slack may be too tight, so the script should use the configured `dedup_ttl_seconds = 5` plus a 1 s safety margin on either side and skip itself with a clear diagnostic if the clock check fails (rather than producing a flaky failure).

The orchestrator must be started/stopped within the script for Scenario D; reuse the harness pattern from TC-162 / TC-163.

## Non-goals

- Exact ledger storage format (the contract is "the ledger persists across restart"; the on-disk shape is an implementation detail).
- TTL behaviour under clock skew or wall-clock adjustment (out of scope; assume monotonic-ish system clock during the test).
- Manual ledger-clearing verbs (out of scope for this slice — SPARQL UPDATE is the v1 maintenance path).
- Cross-subscription dedup (each subscription has its own ledger; no shared state asserted).