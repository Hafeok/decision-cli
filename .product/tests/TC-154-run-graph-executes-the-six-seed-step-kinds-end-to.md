---
id: TC-154
title: run_graph executes the six seed step kinds end-to-end against a fixture VG in an ephemeral env
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-098
  adrs: []
phase: 1
---

## Claim

Calling `core::verify::runner::run_graph(req)` against a fixture `VerificationGraph` containing one step of each seed kind (`shell-command`, `sparql-assertion`, `file-assertion`, `http-request`, `wait-for`, `capture`) executes all six handlers in order, produces a `RunGraphResponse` with verdict `approved`, and persists a `VerificationGraphResult` whose `stepTraces` length is exactly 6 with every outcome `"pass"`.

## Scenarios

### Setup

- Seed env `ENV-FIXTURE-001` with `envType = "ephemeral-tempdir"` and `allowedOps = [shell, filesystem, sparql-local, http]`.
- Start an in-process `wiremock`-style HTTP server bound to a random port; configure one route `GET /health → 200 {"ok": true}`.
- Seed graph `VG-FIXTURE-006` with these six steps in order:
  1. `shell-command` — `echo seeded > seed.ttl && echo "@prefix ex: <urn:ex#> . ex:s ex:p ex:o ." > store.ttl`. Expect exit 0.
  2. `sparql-assertion` — target `store.ttl`, query `SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o }`, `dec:expectRows = 1`.
  3. `file-assertion` — target `seed.ttl`, assert SHA-256 matches the known hash of `"seeded\n"`.
  4. `http-request` — `GET ${health_url}`, `dec:expectStatus = 200`. (The fixture pre-binds `${health_url}` via `capture_bindings` in the request.)
  5. `wait-for` — wrap a `file-assertion` over `marker` that becomes present at t+0.5 s (a sibling test thread `touch`es it). Default poll 1 s, timeout 5 s.
  6. `capture` — `dec:source = "literal"`, `dec:bindName = "summary"`, `dec:value = "all ok"`.

### Assertions

- `run_graph(req)` returns `Ok(response)` with `response.verdict == Verdict::Approved`.
- `response.step_outcomes.len() == 6` and every entry's `outcome == Pass`.
- `response.result` points at a file `.dec/verify/result/VGR-NNN.ttl` that exists on disk and parses as a SHACL-valid `VerificationGraphResult` (round-trip through `StreamWriter` succeeded).
- The persisted result's `dec:stepTraces` list is in graph step order and each entry's `dec:tracesStep` points at the corresponding fixture step IRI.
- `response.emitted_feedback` is empty (no failures).
- The ephemeral tempdir is cleaned up after the call returns (assert the dir does not exist unless `DEC_KEEP_TMP=1` is set in the test process — it must not be).

### Negative path inside the same test

After the positive assertions, repeat the run with a mutated graph: step 2's `dec:expectRows` is changed to `99`. Expect `response.verdict == Verdict::Rejected`, `response.step_outcomes[1].outcome == Fail`, and `response.emitted_feedback.len() == 0` only if step 2's `providesEvidenceFor` is empty in the fixture; if it is non-empty, assert the feedback fan-out per linked TC.

## Runner

`cargo test --test verify_graph_runner_kinds -p decision-cli`. The test lives at `crates/decision-cli/tests/verify_graph_runner_kinds.rs` (top-level integration test so the runner can be exercised end-to-end with a real `StreamWriter` and an in-memory store). The HTTP mock and the temp-dir helper live next to the test.

## Non-goals

- Exhaustive per-kind edge cases (each kind module has its own `kinds/*_tests.rs` unit tests; this TC asserts the *framework* dispatches all six and they each emit a trace).
- Performance — no latency or throughput assertion.
- Concurrency — single-threaded path only; multi-flight protection is the caller's responsibility per FT-098 §Idempotency.
