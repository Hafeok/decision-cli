---
id: TC-084
title: auto-dispatch subscription dedup TTL prevents repeat dispatches on edits
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 2
---

## Premise

`auto_dispatch = true`, dedup TTL = 1 hour. Feature `FT-K` is created, triggering a dispatch event. Within the same hour, the feature's body is updated three times (triggering three feature-update events).

## Acceptance Criteria

- Only the original dispatch event is emitted; the three subsequent updates produce no new events for `(FT-K, ENV-1)`.
- The ledger entry for `(FT-K, ENV-1)` reflects the timestamp of the original dispatch.
- After the TTL elapses (simulated by advancing clock or directly aging the ledger), a feature-update event fires a fresh dispatch.
- Setting TTL to 0 in config causes every event to dispatch (validates the testing override).

## Notes

Without dedup, a developer iterating on a feature spec would cause an event storm of redundant proposals — defeating the worker's value with noise. TTL bounded by the ledger keeps the subscription useful under realistic edit cadences.
