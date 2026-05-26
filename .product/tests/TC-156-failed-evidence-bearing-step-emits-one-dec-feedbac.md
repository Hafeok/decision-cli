---
id: TC-156
title: failed evidence-bearing step emits one dec:Feedback per linked TC with the correct class
type: scenario
status: passing
validates:
  features:
  - FT-098
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --test tc_156_failed_evidence_bearing_step_emits_one_dec_feedbac
runner-timeout: 120
last-run: 2026-05-26T13:38:44.856415896+00:00
last-run-duration: 0.3s
---

## Claim

When a step with `dec:providesEvidenceFor [TC-A, TC-B]` finishes with `outcome = fail`, the runner emits exactly two `dec:Feedback` artifacts — one targeting TC-A, one targeting TC-B — each with `dec:class = "regression"`, `dec:fromActivity = run_activity`, and a body containing the step's expected-vs-actual one-liner; when the outcome is `unrunnable`, the class is `"gap"` instead.

## Scenarios

### Setup

- Seed env `ENV-FIXTURE-001` (`ephemeral-tempdir`).
- Seed graph `VG-FIXTURE-FB` with two steps:
  1. `shell-command` `dec:command = "exit 1"`, `dec:expectExitCode = 0`, `dec:providesEvidenceFor = [TC-EVI-A, TC-EVI-B]`. (Will fail.)
  2. `sparql-assertion` `dec:target = "missing.ttl"`, `dec:providesEvidenceFor = [TC-EVI-C]`. (Will be unrunnable — target missing.)

### Assertions — failed step (class = regression)

- `response.emitted_feedback` contains exactly two IRIs whose persisted artifacts have:
  - `dec:targetTc = TC-EVI-A` (one entry) and `dec:targetTc = TC-EVI-B` (the other).
  - `dec:class = "regression"`.
  - `dec:fromActivity` equal to the `run_activity` IRI on the request.
  - Body containing the literal substring `"expected exit 0, got 1"` (the runner's standard expected-vs-actual line for shell-command failures).

### Assertions — unrunnable step (class = gap)

- `response.emitted_feedback` contains one further IRI whose artifact has:
  - `dec:targetTc = TC-EVI-C`.
  - `dec:class = "gap"`.
  - `dec:fromActivity` equal to `run_activity`.
  - Body containing the substring `"target missing"` or `"could not load target"` (runner-defined; the test allows either canonical form via regex).

### Assertions — no feedback for passing or non-evidence-bearing steps

- If the fixture is amended with a third step that *passes* and has `providesEvidenceFor = [TC-EVI-D]`, no `Feedback` artifact targeting TC-EVI-D is emitted. The test must confirm this by asserting `response.emitted_feedback.iter().none(|f| f.target == TC-EVI-D)`.

## Runner

`cargo test --test verify_graph_runner_feedback -p decision-cli`. Lives at `crates/decision-cli/tests/verify_graph_runner_feedback.rs`. Uses the FT-031 emit-feedback SDK helpers under the hood (no duplicate implementation).

## Non-goals

- Feedback routing to upstream roles (FT-029 covers that — this TC asserts emission, not delivery).
- Feedback lifecycle state transitions (FT-027 covers those).
- The body's prose formatting beyond the asserted substring (rendering is allowed to evolve without breaking this TC).