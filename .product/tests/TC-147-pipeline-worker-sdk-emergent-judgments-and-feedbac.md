---
id: TC-147
title: 'pipeline-worker SDK: Emergent judgments and feedback emission via side-channel — exit criterion'
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-147-pipeline-worker-sdk-side-channel.sh
runner-timeout: 120
last-run: 2026-05-25T23:43:37.401105492+00:00
last-run-duration: 0.7s
---

## Description

Exit criterion for [FT-082](FT-082). Verifies the two side-channel APIs the
worker SDK exposes on `Session`:

1. **`session.record_emergent_judgment(decision, rationale)`** — produces
   triples that ride on the artifact emission set and surface to the paired
   interpretation session's bundle. Reachable via SPARQL on the completion
   payload's N-Quads body. Refuses calls after the session is closed,
   refuses blank inputs.

2. **`session.emit_feedback(class, severity, evidence, blocking=...)`** —
   produces a `dec:Feedback` artifact via the side-channel store
   (single transport — no separate channel, per ADR-022).
   - `blocking=True` (or a class whose ADR-023 default is blocking) forces
     `session.build_completion()` to return `outcome=blocked` (ADR-025) and
     drops the half-formed artifact triples while preserving the Feedback.
   - `blocking=False` keeps `outcome=success` and ships both the main
     artifact and the Feedback in the same completion payload.

## Test plan

Driven by `pytest` against `workers/pipeline-worker-sdk/tests/
test_tc_147_side_channel_emissions.py` through the bash runner
`tests/scripts/tc-147-pipeline-worker-sdk-side-channel.sh`. The suite
exercises:

- emergent-judgment quad shape, identity, blank-input rejection,
  post-completion guard;
- blocking feedback ⇒ `outcome=blocked` + artifact triples dropped;
- non-blocking feedback ⇒ `outcome=success` + both artifacts shipped;
- per-emission disposition override only recorded when diverging from
  the class default;
- multiple emissions in one session (blocking wins);
- `FeedbackEmission` Pydantic validation (≥ 20 chars evidence; controlled
  severity vocabulary).

Pass condition: every test in the suite passes; the bash runner exits 0.