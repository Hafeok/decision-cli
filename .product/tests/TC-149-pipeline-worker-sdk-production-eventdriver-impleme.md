---
id: TC-149
title: 'pipeline-worker SDK: Production EventDriver implementation — exit criterion'
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-149-pipeline-worker-sdk-event-driver.sh
runner-timeout: 120
last-run: 2026-05-25T23:43:38.818441635+00:00
last-run-duration: 0.7s
---

## Description

Exit criterion for FT-084: the production `EventDriver` wires FT-077's
SSE consumer + atomic claim + completion POST + catalog cache to
FT-078's pyoxigraph-backed `Session` lifecycle. The driver advertises
capability tags, walks the SSE stream, claims each matching dispatch
atomically, hands a `Session` to worker code via the `Driver` Protocol
(FT-083), and POSTs the resulting completion back through the wire
layer.

The test suite at
`workers/pipeline-worker-sdk/tests/test_tc_149_event_driver.py`
exercises every guarantee the feature_spec names:

1. **End-to-end dispatch → completion lifecycle.** An EventDriver
   instance subscribes to an in-memory fake harness (httpx
   `MockTransport`, same shape as TC-142), receives a dispatch event
   for its advertised capability tag, claims it, emits an artifact
   triple via the Session, and POSTs a completion that the harness
   accepts with HTTP 200.
2. **Transient SSE disconnect resumes via `Last-Event-ID`.** Mid-stream
   the SSE response closes cleanly after event id 11; the driver
   reconnects transparently and the harness only replays events
   strictly newer than 11.
3. **Transient SSE 5xx / network error retries within policy.** The
   driver retries the SSE GET up to its `sse_reconnect_policy.max_attempts`
   before propagating the last transport exception.
4. **Transient completion POST failure retries with backoff and
   succeeds.** A 503 → 503 → 200 sequence produces exactly one entry in
   the harness's completion log.
5. **Permanent completion failure surfaces `CompletionFailed`.**
   Exhausting the retry policy raises `CompletionFailed` out of
   `driver.complete(payload)` so the worker treats it as a hard stop.
6. **4xx rejection (SHACL violation) surfaces `CompletionRejected`.**
   The harness's deterministic rejection propagates with the status
   code so the worker can act on the validation report.
7. **Lost claims skip the dispatch silently.** A pre-claimed dispatch
   never yields a Session to worker code; the driver advances to the
   next SSE envelope.
8. **Lifecycle hooks behave correctly.** `aclose` is idempotent;
   `complete()` on a closed driver raises `RuntimeError`; `async with`
   closes on clean exit AND posts a best-effort blocked completion on
   worker crash so the harness sees the session terminate.

Plus the structural contract the EventDriver inherits from FT-083:
`isinstance(driver, Driver)` works, and worker code written against the
`Driver` Protocol runs unchanged under both `FakeDriver` (TC-148) and
`EventDriver` (this suite).

Runner: `bash tests/scripts/tc-149-pipeline-worker-sdk-event-driver.sh`