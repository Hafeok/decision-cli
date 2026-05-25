---
id: FT-084
title: 'pipeline-worker SDK: Production EventDriver implementation'
phase: 3
status: planned
depends-on:
- FT-077
- FT-078
- FT-083
adrs: []
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:pipeline-worker-slice-1`. The concrete production `Driver`
(FT-083 interface): subscribes to pipeline-cli's SSE endpoint, issues atomic
claims, hands sessions to worker code, posts completions. Composed from the
wire layer (FT-077) and the session lifecycle (FT-078).

## Location

`workers/pipeline-worker-sdk/src/pipeline_worker_sdk/driver/event_driver.py`.

## Scope

- `EventDriver` implementing the `Driver` protocol from FT-083.
- Wires FT-077 (SSE + POST) to FT-078 (Session lifecycle).
- Handles the dispatch → claim → session → completion path end-to-end.
- Surfaces wire-level errors as session-level outcomes (`blocked`,
  `escalated`, transport failures retried with backoff).

## Out of scope

- Anything in the layers it composes (those are FT-077 and FT-078).
- Worker process lifecycle / supervisor concerns (the
  `pipeline-cli workers run` subcommand owns that, not the SDK).

## Success criteria

- An EventDriver instance, given an `LITELLM_BASE_URL` and a harness SSE
  endpoint, runs one dispatch → completion cycle end-to-end against a live
  pipeline-cli.
- Transient SSE disconnect mid-session resumes correctly; transient POST
  failure on completion retries with backoff and eventually succeeds (or
  surfaces a permanent failure to the operator).