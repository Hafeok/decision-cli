---
id: TC-155
title: runtime safety re-check aborts the run and writes a rejected VGR when an env mutation invalidates allowedOps
type: scenario
status: passing
validates:
  features:
  - FT-098
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test tc_155_runtime_safety_re_check_aborts_the_run_and_writes
runner-timeout: 120
last-run: 2026-05-28T08:48:42.998264087+00:00
last-run-duration: 0.5s
---

## Claim

When `run_graph` is invoked against a `VerificationGraph` whose steps require an op not present in the env's `allowedOps` (a state reachable only via mutation after authoring), Phase 1's `runtime_safety` re-check aborts before any step runs, returns `Err(RunnerError::SafetyViolation { step, op })`, **and** writes a `VerificationGraphResult` with `verdict = rejected`, rationale `"safety: step <S> requires op <O> not in env.allowedOps"`, and empty `stepTraces`.

## Scenarios

### Setup

- Seed graph `VG-FIXTURE-SAFETY` with one `http-request` step (`dec:requiredOps = [http]`) targeting `dec:verifies = FT-FIXTURE`.
- Seed env `ENV-FIXTURE-SAFETY` initially with `allowedOps = [http, filesystem]`. The authoring-time gate (FT-037) allowed the graph because at write time the subset check passed.
- **Mutate the env after authoring**: rewrite `ENV-FIXTURE-SAFETY.ttl` (or directly edit the in-store projection in the test) so that `allowedOps = [filesystem]` only. The `http` op is now disallowed.

### Assertions

- `run_graph(req)` returns `Err(RunnerError::SafetyViolation { step: VG-FIXTURE-SAFETY/step/0, op: "http" })`.
- A `VerificationGraphResult` is persisted at `.dec/verify/result/VGR-NNN.ttl` with:
  - `dec:resultOf = VG-FIXTURE-SAFETY`.
  - `dec:verdict = "rejected"`.
  - `dec:rationale` contains the literal substring `"safety: step"` and the op name `"http"`.
  - `dec:stepTraces` is an empty `rdf:List`.
- No `dec:Feedback` artifact is emitted (Phase 1 abort precedes Phase 5 feedback emission; the operator sees the rejected result and acts on the env mismatch).
- The `runtime_safety` predicate function called from `runner/runtime_safety.rs` and `core::verify::safety` (used at authoring time) is the **same function**, not a duplicate — the test asserts this by reflection or by importing the same symbol in both call sites (a separate assertion can call `core::verify::safety::check_ops_subset(...)` and verify it returns the same result the runner observed).

## Runner

`cargo test --test verify_graph_runner_safety -p decision-cli`. Lives at `crates/decision-cli/tests/verify_graph_runner_safety.rs`. The env-mutation step uses a direct store write through `StreamWriter` (or, if SHACL would reject the mutated env, via a privileged test helper that bypasses validation — the point is to simulate a mutation that occurred outside the gate).

## Non-goals

- The authoring-time gate (FT-037 covers that).
- HTTP transport behaviour — the step never runs.
- Concurrent-mutation races between graph load and step dispatch (out of scope; this slice asserts the deterministic happy-path of "graph loaded, env loaded, gate runs, abort if violation").