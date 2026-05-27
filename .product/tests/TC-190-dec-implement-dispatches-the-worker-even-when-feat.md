---
id: TC-190
title: dec implement dispatches the worker even when feature status is complete if defect feedback is outstanding
type: exit-criteria
status: passing
validates:
  features:
  - FT-108
  adrs:
  - ADR-031
phase: 3
runner: cargo-test
runner-args: tc_190_implement_gate_falls_through_on_outstanding_feedback
runner-timeout: 60
last-run: 2026-05-27T09:13:24.586155434+00:00
last-run-duration: 0.6s
---

## Claim

The `dec implement FT-XXX` dispatch gate respects two signals:

- `feature.status = complete` AND no outstanding implementer-targeted defect feedback → short-circuit, no worker dispatch (today's behaviour, preserved).
- `feature.status = complete` AND outstanding `produced`-state defect feedback exists for the feature's TCs with `targetRole = implementer` → fall through to worker dispatch.

## Scenarios

### Setup A (preserved short-circuit)

- Feature `FT-T190a` with status `complete`.
- No outstanding defect feedback for its TCs.

Call `dec implement FT-T190a`. Assert:

1. The worker subprocess is NOT spawned (use the stub-runner instrumentation hook).
2. The handler returns an outcome whose `worker_status = "skipped:already-complete"` (or the equivalent today's behaviour produces).

### Setup B (new fall-through)

- Feature `FT-T190b` with status `complete`.
- One pre-seeded defect feedback with `class=defect, targetRole=implementer, lifecycleState=produced`, source_artifact one of FT-T190b's TCs.

Call `dec implement FT-T190b`. Assert:

1. The worker IS dispatched (subprocess counter advances OR the mock fires).
2. The bundle delivered to the worker carries the seeded feedback in `defect_feedback`.

### Boundary

- An `addressed` (terminal) feedback does NOT trigger fall-through — only `produced` does.