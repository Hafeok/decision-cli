---
id: FT-082
title: 'pipeline-worker SDK: Emergent judgments and feedback emission via side-channel'
phase: 3
status: planned
depends-on:
- FT-078
adrs: []
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. Implements the two side-channel
APIs from `docs/ddd/Implementing_DDD.md` §6: emergent judgment recording (for
in-authority calls the worker makes during execution) and feedback emission
(for out-of-authority issues that need to escalate upstream).

## Location

`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/side_channel/` — both
APIs are exposed as methods on `Session` (from FT-078) but the emission/
packaging logic lives in this module.

## Scope

- `session.record_emergent_judgment(decision, rationale)`:
  - For in-authority judgments the worker makes mid-session.
  - Triples land in the artifact's metadata.
  - Surfaced to the paired interpretation session (FT-019…FT-025 verifier
    pipeline) for review.
- `session.emit_feedback(class, severity, evidence, blocking=False)`:
  - Emits a `Feedback` artifact conforming to the feedback schema
    (FT-026 / ADR-022).
  - `class` drawn from the controlled vocabulary (ADR-023).
  - `blocking=True` causes the session to exit with `outcome=blocked` (per
    ADR-025); non-blocking feedback flows alongside `outcome=completed`.
- Both APIs emit triples into the session's emission set, packaged into the
  completion event alongside the main artifact (no separate transport).

## Out of scope

- Persisting feedback locally on the worker (the harness owns durable
  feedback state — see FT-027 / FT-029).
- Routing decisions (the harness routes per ADR-026; worker only emits).

## Success criteria

- A worker calling `record_emergent_judgment` produces triples visible in the
  paired interpretation session's bundle.
- A worker calling `emit_feedback(blocking=True)` ends the session with
  `outcome=blocked` and the feedback artifact is included in the completion
  payload.
- A worker calling `emit_feedback(blocking=False)` does not affect session
  outcome but produces a Feedback artifact in the completion.