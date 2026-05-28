---
id: TC-160
title: dec verify feature exits 3 when any TC has no covering graph even if covered TCs all pass
type: scenario
status: passing
validates:
  features:
  - FT-099
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-160-dec-verify-feature-coverage-gap.sh
runner-timeout: 180
last-run: 2026-05-28T08:48:45.960727560+00:00
last-run-duration: 0.8s
---

## Claim

`dec verify feature <FT>` exits with code **3** when at least one TC listed in the feature's `dec:tests` has no covering `VerificationGraph` — regardless of whether the other covered TCs all passed. The output names the uncovered TCs in a `Coverage gaps:` block so the operator cannot mistake an "approved" line for a complete signal.

## Scenarios

### Setup

- Fresh `.dec/` initialised via `dec init`.
- Seed feature `FT-GAP` with TCs `[TC-COV-A, TC-COV-B, TC-COV-C]`.
- Seed env `ENV-001`.
- Seed **only one** graph `VG-COV-AB` covering `[TC-COV-A, TC-COV-B]` (both passing). `TC-COV-C` is uncovered.

### Scenario A — covered TCs pass, one uncovered

Invoke `dec verify feature FT-GAP`. Assertions:

- Exit code: **3** (distinct from `rejected = 1` so CI can branch on "missing graph" vs "graph failed").
- Stdout contains a per-graph row `VG-COV-AB (ENV-001) → approved`.
- Stdout contains a per-TC table with three rows: `TC-COV-A approved`, `TC-COV-B approved`, `TC-COV-C rejected (no covering verification graph result)`.
- Stdout contains `Coverage gaps: TC-COV-C` (or equivalent — the gap list is non-empty and includes TC-COV-C).
- Stdout contains `Aggregate verdict: rejected` (per FT-097's aggregation: an uncovered TC contributes `rejected` per the empty-set rule, and rejection dominates).
- Suggestion text on stdout/stderr points at `dec verify graph generate FT-GAP` as the remedy (mirrors FT-049's hint pattern).

### Scenario B — all TCs uncovered

Remove the existing graph (or seed a fresh feature `FT-EMPTY` with TCs but no graphs at all). Invoke `dec verify feature FT-EMPTY`. Assertions:

- Exit code: 3.
- `Coverage gaps:` block lists every TC in the feature.
- The per-graph table is empty (no rows) or contains a single explanatory row `(no covering graphs found)`.
- `Aggregate verdict: rejected` with rationale `"no verification graph result covers <FT-EMPTY>"` or `"X of Y TCs have no covering graph"`.

### Cross-check against approved-with-coverage

For contrast, a third invocation against TC-159's `FT-FIXTURE` (fully covered, all pass) must exit 0 and contain `Coverage gaps: none`. This run does not need to be in this TC's script — but the script should assert that the *exit-code disambiguation* (3 vs 0 vs 1) is meaningful by running both at minimum.

## Runner

`bash tests/scripts/tc-160-dec-verify-feature-coverage-gap.sh`. Same fixture-store pattern. The script must run both Scenario A and Scenario B and the cross-check, asserting the exit code on each.

## Non-goals

- Coverage-waiver handling — when an [ADR-031](ADR-031) `CoverageWaiver` exists for the feature, behaviour is out of scope for this TC (a sibling TC in a later slice covers waiver interaction).
- Per-env coverage tightening (ADR-031 deferred this to a later slice; for now coverage means "some env covers each TC").
- The chain-integrity dispatch gate's behaviour (FT-047 covers that — it consumes the coverage primitive directly, not this verb).