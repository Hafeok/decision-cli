---
id: TC-011
title: sse_dispatch_event_delivered_within_one_second
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-011-sse-latency.sh
runner-timeout: 120
last-run: 2026-05-19T09:46:07.737086591+00:00
last-run-duration: 0.1s
---

## Purpose

Exit criterion for FT-004 SSE delivery: a remote Python worker (FT-013) must receive a dispatch event within **one second** of emission from FT-003 via the SSE transport. The 1-second budget is the measurable phase-completion threshold for slice 1.

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #11.

## Given

- A running `dec` instance with the SSE endpoint (FT-004) bound to localhost.
- A Python worker (FT-013) connected to the SSE endpoint, subscribed and idle.
- Both processes co-located on the same host (slice 1 binds SSE to localhost only).

## When

A dispatch event is emitted through FT-003 (triggered by, e.g., `dec implement FT-XXX`). Record `t_emit` at the moment the event is persisted with `published = false` and `t_recv` at the moment the worker reads the framed SSE record from its socket.

## Then

1. `t_recv − t_emit < 1.000 s` for **every** event in a sample of N ≥ 10 successive dispatches.
2. The event seq received over SSE matches the seq in the events graph (FT-005 replay over the same range returns identical seq values).
3. No events are missed: the count received over the wire equals the count of `oxi:Event` rows added in the interval.

## Notes

- The 1-second budget is the slice 1 contract; later slices may tighten it.
- Heartbeats and reconnects are not exercised here (latency budget is for steady-state delivery).
- Implementations should record timestamps in the same clock (e.g., monotonic from `clock_gettime(CLOCK_MONOTONIC)`-equivalent) on the harness side and compare against the worker's reception time.

## Formal specification

⟦Λ:ExitCriteria⟧{
  sse_delivery_latency_p100 < 1.000s
  sse_delivery_completeness = 1.0
  sse_seq_continuity = monotonic_strictly_increasing
  sample_size ≥ 10
}

⟦Ε⟧⟨δ≜0.9;φ≜80;τ≜◊⁺⟩