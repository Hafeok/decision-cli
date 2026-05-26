---
id: TC-157
title: stopOnFail truncates execution but every step still has a trace entry equal to graph.steps length
type: scenario
status: unimplemented
validates:
  features:
  - FT-098
  adrs: []
phase: 1
---

## Claim

When a `shell-command` step with `dec:stopOnFail = true` fails, the runner halts the step loop early **and** still emits a `VerificationStepTrace` for every subsequent step (with `outcome = "unrunnable"` and `errorMessage = "skipped: prior step <N> halted the run"`), so the persisted `VerificationGraphResult.dec:stepTraces` length equals `graph.dec:steps` length exactly — preserving the invariant that downstream consumers (SPARQL, dashboards) can rely on positional alignment between graph steps and result traces.

## Scenarios

### Setup

- Seed env `ENV-FIXTURE-001` (`ephemeral-tempdir`).
- Seed graph `VG-FIXTURE-STOP` with four steps:
  1. `shell-command` `echo ok` (pass).
  2. `shell-command` `exit 1`, `dec:expectExitCode = 0`, `dec:stopOnFail = true` (fail; halts the loop).
  3. `shell-command` `echo never` (would pass, but is skipped).
  4. `sparql-assertion` over `store.ttl` (would be unrunnable since step 1 didn't create it, but is skipped before reaching that check).

### Assertions

- `response.verdict == Verdict::Rejected` (failure on step 2; the rule from FT-097 applies).
- `response.step_outcomes` has length **4** — one per graph step — with outcomes `[Pass, Fail, Unrunnable, Unrunnable]`.
- The persisted `VerificationGraphResult.dec:stepTraces` Turtle list also has length 4 and each entry's `dec:tracesStep` matches the corresponding graph step IRI in order (positional alignment preserved).
- Step 3's trace has `dec:errorMessage` containing the literal substring `"skipped: prior step 1 halted the run"` (zero-indexed; step 2 in human counting = index 1).
- Step 4's trace has the same skipped-message pattern.
- No `shell-command` execution attempt is logged for steps 3 or 4 (assert via observable side effect: step 3 would `echo never` — verify `never` does **not** appear in any captured stdout).

### Negative cross-check

Same fixture but step 2 has `dec:stopOnFail = false` (or absent). Expect all four steps to attempt execution, the trace length is still 4, but step 3's outcome is `Pass` and step 4's outcome is `Unrunnable` (because step 1 didn't create `store.ttl` — the unrunnable comes from step 4's own check, not from being skipped). This asserts that the stop-on-fail behaviour only kicks in when explicitly set.

## Runner

`cargo test --test verify_graph_runner_stop_on_fail -p decision-cli`. Lives at `crates/decision-cli/tests/verify_graph_runner_stop_on_fail.rs`. Same fixture-store setup as TC-154 / TC-155 / TC-156.

## Non-goals

- The semantics of `stopOnFail` on non-shell kinds (out of scope for v1; only `shell-command` supports the predicate in this slice — other kinds' SHACL shape rejects the field).
- Resumption after stop (not supported; re-running produces a new VGR per FT-098 §Idempotency).
- Per-step timeout interaction with stop-on-fail (separate concern; the timeout sets the step's outcome but the stop-on-fail check still runs on the resulting outcome).
