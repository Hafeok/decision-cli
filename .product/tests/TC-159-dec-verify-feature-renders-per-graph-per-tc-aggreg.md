---
id: TC-159
title: dec verify feature renders per-graph + per-TC + aggregate verdict and maps aggregate to exit code
type: exit-criteria
status: passing
validates:
  features:
  - FT-099
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-159-dec-verify-feature.sh
runner-timeout: 240
last-run: 2026-05-28T08:48:45.960727560+00:00
last-run-duration: 1.2s
---

## Claim

`dec verify feature <FT>` enumerates every graph that verifies the feature (or whose `providesEvidenceFor` chain touches its TCs), runs each `(graph, env)` tuple through `core::verify::runner::run_graph` sequentially, composes the results via `core::verify::aggregate::aggregate_verdict`, prints a per-graph table + per-TC verdict table + aggregate verdict block, and maps the aggregate verdict to exit code (0 / 1 / 2) — with no coverage gap present.

## Scenarios

### Setup

- Fresh `.dec/` initialised via `dec init`.
- Seed feature `FT-FIXTURE` with TCs `[TC-EVI-A, TC-EVI-B]`.
- Seed env `ENV-001 (ephemeral-cli)`.
- Seed two graphs both targeting `FT-FIXTURE` in `ENV-001`:
  - `VG-FT-1` covers TC-EVI-A with a passing shell-command + sparql-assertion.
  - `VG-FT-2` covers TC-EVI-B with a passing shell-command.

### Scenario A — full approved

Invoke `dec verify feature FT-FIXTURE`. Assertions:

- Exit code: 0.
- Stdout contains exactly one row per graph in the per-graph table: `VG-FT-1 (ENV-001) → approved` and `VG-FT-2 (ENV-001) → approved`.
- Stdout contains a per-TC table with two rows: `TC-EVI-A approved (covered by VG-FT-1 pass)` and `TC-EVI-B approved (covered by VG-FT-2 pass)`.
- Stdout contains `Coverage gaps: none`.
- Stdout contains `Aggregate verdict: approved`.

### Scenario B — one graph rejected

Mutate `VG-FT-2`'s step to expect a different exit code so it fails. Re-invoke `dec verify feature FT-FIXTURE`. Assertions:

- Exit code: 1 (rejection dominates).
- Per-graph table shows `VG-FT-2 → rejected`.
- Per-TC table shows `TC-EVI-B rejected`.
- `Aggregate verdict: rejected`.

### Scenario C — sequential execution

Both scenarios above must complete in single-threaded ordering — i.e. the runner is invoked once per `(graph, env)`, in deterministic order, with no parallelism in v1. Assert this by capturing the `Session` artifacts created during the run (`dec session list --since <start>`): there must be exactly N+1 sessions (one aggregate + N per-graph), and the per-graph sessions' `dcterms:created` timestamps must be strictly monotonically non-decreasing.

### Scenario D — --format json

Invoke `dec verify feature FT-FIXTURE --format json`. Assertions:

- Stdout parses as JSON with keys `session_id`, `per_graph`, `per_tc`, `coverage_gaps`, `aggregate`.
- `per_graph` is a length-2 array.
- `per_tc` is a length-2 array; each entry has `tc`, `verdict`, `rationale`, `from_results`.
- `coverage_gaps` is an empty array.
- `aggregate.verdict` matches the scenario.

## Runner

`bash tests/scripts/tc-159-dec-verify-feature.sh`. Same temp-`.dec/` setup pattern as TC-158. Each scenario is a separate invocation against the same fixture store; the script resets the store (or starts from scratch) between scenarios that mutate it.

## Non-goals

- Parallel execution (`--parallel N` is out of scope for v1).
- Cross-environment de-duplication (the function takes whatever results the caller hands it; see TC-153).
- Coverage gap exit-code path (TC-160 covers that).
- Dry-run path (TC-161 covers that).