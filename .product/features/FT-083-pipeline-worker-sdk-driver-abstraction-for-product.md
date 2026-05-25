---
id: FT-083
title: 'pipeline-worker SDK: Driver abstraction for production and replay'
phase: 3
status: planned
depends-on: []
adrs: []
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. Defines the `Driver` interface
that both EventDriver (FT-084, production) and ReplayDriver (slice 2, offline
replay) implement. Workers consume sessions via `async for session in driver:`
and never know which driver invoked them.

This is what operationalizes "per-role queries are the unit of evolution"
(`docs/ddd/Implementing_DDD.md` §4) — the same worker code runs against
historical bundles offline as runs against live dispatches.

## Location

`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/driver/` —
`base.py` for the protocol/ABC, `fake.py` for the in-memory test double.

## Scope

- The `Driver` protocol/ABC:
  - `__aiter__` / `__anext__` yielding `Session` objects
  - lifecycle hooks for clean shutdown
  - completion handoff (the worker calls back through the driver to post)
- Test doubles: an in-memory `FakeDriver` for SDK unit tests, accepting a
  pre-built list of `(bundle, expected_completion)` tuples.
- Documentation of the contract: what is and isn't observable to worker code
  about which driver is in use.

## Out of scope

- ReplayDriver implementation (slice 2 work).
- Concrete EventDriver implementation (split out as FT-084 so this Feature
  keeps a clean interface-only boundary).

## Success criteria

- A worker written against `Driver` runs unchanged under the FakeDriver in
  SDK tests and under EventDriver in integration.
- Type-checker rejects worker code that branches on driver implementation
  type.