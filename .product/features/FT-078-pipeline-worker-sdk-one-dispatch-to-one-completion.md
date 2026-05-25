---
id: FT-078
title: 'pipeline-worker SDK: One dispatch to one completion lifecycle with in-memory pyoxigraph store'
phase: 3
status: planned
depends-on:
- FT-077
- FT-069
- FT-073
adrs:
- ADR-049
- ADR-050
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. The Session is the unit of
measurement on the worker side and mirrors the harness's session record on the
other side of the wire. Addresses ADR-049 (pyoxigraph in-memory store) and
ADR-050 (Session IS a `prov:Activity`).

Depends on the dual-provenance discipline (FT-069 mechanical-provenance SHACL
and FT-073 GraphWriter enforcement, governed by ADR-038 and ADR-041): the
session record on the harness side becomes the `prov:Activity` whose URI this
in-process Session shares, and mechanical provenance triples on produced
artifacts are populated by the harness's GraphWriter — not by the worker.

## Location

`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/session.py` (and tests
under `workers/pipeline-worker-sdk/tests/`).

## Scope

- One `Session` object per dispatch, lifecycle bound to the dispatch lifetime.
- Owns an in-memory `pyoxigraph.Store` initialized from the dispatch's bundle
  N-Quads payload. The store holds the session's sub-graph for the duration of
  the call and is discarded on completion.
- Accumulates telemetry across all provider calls and side-channel emissions.
- On clean exit: serializes artifact triples + side-channel triples + telemetry
  into a completion payload (handed to FT-077 wire layer to post).
- On exception: emits whatever side-channel triples were captured and posts a
  `blocked` or `escalated` completion (no silent drops).
- The Session IS a `prov:Activity` (ADR-050). Mechanical provenance annotations
  on produced artifacts (`prov:wasGeneratedBy`, `prov:wasAttributedTo`,
  `prov:used`) are populated by the harness's GraphWriter from the session
  record at write time (FT-073 enforces SHACL, FT-069 ships the fragment) —
  the worker does not duplicate these on the wire.

## Out of scope

- Per-session persistence (sessions are ephemeral on the worker; the harness
  owns the durable session record).
- Multi-dispatch sessions / batching (one dispatch ⇒ one session).
- Mechanical-provenance triple emission from the worker side (owned by FT-069
  / FT-073 on the harness side).

## Success criteria

- A dispatch with N bundle triples produces a Session whose `pyoxigraph.Store`
  contains exactly those N triples on entry.
- On clean completion, the completion payload contains: artifact triples,
  side-channel triples (if any), and the telemetry block.
- On uncaught exception inside worker code, the SDK posts a `blocked`
  completion with the captured side-channel triples rather than dropping them.
- The Session's URI is the same URI used by the harness's `prov:Activity`
  record for that dispatch — verifiable by FT-075's full-chain provenance
  query.