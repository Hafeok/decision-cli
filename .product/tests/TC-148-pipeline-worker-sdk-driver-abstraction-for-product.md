---
id: TC-148
title: 'pipeline-worker SDK: Driver abstraction for production and replay — exit criterion'
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-148-pipeline-worker-sdk-driver.sh
runner-timeout: 120
last-run: 2026-05-25T23:43:38.105648678+00:00
last-run-duration: 0.7s
---

## Description

Exit criterion for FT-083: validates the `Driver` Protocol/ABC abstraction
that lets worker code consume sessions via `async for session in driver:`
without knowing whether the dispatch came from the production EventDriver
(FT-084) or from a future ReplayDriver (slice 2).

Drives `workers/pipeline-worker-sdk/tests/test_tc_148_driver_abstraction.py`
via `tests/scripts/tc-148-pipeline-worker-sdk-driver.sh`. The suite covers:

1. The `Driver` Protocol's surface is minimal — iteration, lifecycle, and
   the `complete(payload)` handoff — and is `runtime_checkable`.
2. The `FakeDriver` accepts a pre-built list of `(bundle, expected_completion)`
   tuples (or bare `DispatchEvent`s) and records every completion the worker
   hands back, so SDK unit tests can assert on shape.
3. A worker body written against `Driver` (not `FakeDriver` or `EventDriver`)
   runs unchanged under the FakeDriver — establishing the interchangeability
   property that makes EventDriver (FT-084) and ReplayDriver (slice 2)
   drop-in replacements in tests vs. production.
4. The Protocol does not leak FakeDriver-specific attributes
   (`received_completions`, `dispatches`, `closed`, …) so worker code cannot
   silently branch on driver implementation type.
5. Lifecycle hooks are well-behaved: `aclose()` is idempotent, a closed
   driver stops yielding sessions, and `async with driver:` calls `aclose`
   on both normal exit and exception paths.