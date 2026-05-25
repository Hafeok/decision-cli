---
id: TC-142
title: 'pipeline-worker SDK: SSE consumer and HTTP poster for the dispatch/completion protocol — exit criterion'
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-142-pipeline-worker-sdk-wire.sh
runner-timeout: 120
last-run: 2026-05-25T21:20:52.644704677+00:00
last-run-duration: 0.3s
---

## Description

Exit criterion for FT-077: the pipeline-worker SDK's wire layer exposes
a working dispatch/completion protocol against an in-memory fake harness
that mimics the real SSE + HTTP POST surface (ADR-045) and capability
catalog (ADR-033).

The test suite at
`workers/pipeline-worker-sdk/tests/test_tc_142_wire_protocol.py`
exercises the three success criteria the feature_spec names:

1. **End-to-end lifecycle.** A worker advertising a capability tag
   subscribes via SSE, receives a matching `dispatch` event, claims it
   atomically, and posts an N-Quads-bearing completion that the
   harness accepts with HTTP 200.
2. **`Last-Event-ID` resume.** After a disconnect mid-stream the SDK
   reconnects and the harness only replays events strictly newer than
   the last delivered id; the SDK preserves the cursor across
   `dispatches()` invocations.
3. **Concurrent claim resolution.** Two workers claim the same
   dispatch in parallel; exactly one sees `won=True` and the other
   receives 409 with `reason=already-claimed`. The winning worker's
   second claim is idempotent.

Additional coverage:

- The completion poster retries 5xx responses with bounded
  exponential backoff and surfaces `CompletionFailed` once exhausted.
- The model-catalog cache resolves capability tags from a single HTTP
  fetch and never refetches while the TTL is fresh.
- The SSE consumer drops envelopes whose capability tag is not in the
  worker's advertised set.

Runner: `pytest workers/pipeline-worker-sdk/tests/test_tc_142_wire_protocol.py`