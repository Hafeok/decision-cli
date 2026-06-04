---
id: TC-372
title: add-judge-worker coherence audit fails when agent_loop references a field absent from pydantic_io_models
type: scenario
status: unimplemented
validates:
  features:
  - FT-139
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-372-cluster-audit-judge-worker-negative.sh
runner-timeout: 60
observes:
- exit-code
---

## Acceptance criteria

Negative-case teeth test for the `add-judge-worker` coherence audit. Verifies that [FT-139](FT-139) §Phase 4's `scripts/checks/cluster-audit-add-judge-worker.py` catches a divergence the broad worker would have caught implicitly via shared context — the **load-bearing property** of [ADR-080](ADR-080)'s decomposition pattern.

### Conditions

Bash runner script against a synthetic-negative cell-output fixture under `tests/fixtures/cluster-audit-add-judge-worker/negative-field-mismatch/`:

- `pydantic_io_models.py` declares `class JudgeInput { feature_id: str; tc_id: str }`.
- `agent_loop.py` references `payload.feature_spec_body` — a field absent from the model.

Setup:
- Invoke `python3 scripts/checks/cluster-audit-add-judge-worker.py <fixture-dir>`.

Assertions:
- Exit code is `1` (audit failure, not unrunnable).
- stderr contains the check identifier verbatim (e.g. `check=agent_loop_field_coverage`).
- stderr names the offending field `feature_spec_body` and the file `agent_loop.py`.

### Why this matters

The broad worker holds the input schema and the consuming code in one context — drift is impossible by construction. The decomposed cluster cannot rely on that; the audit IS the replacement guarantee. If this TC ever passes without the audit catching the field drift, the cluster pattern is silently weaker than the monolith — exactly the failure mode ADR-080 §Rejected alternatives "Cell-cluster without coherence audit" foreclosed.

### Exit codes

- `0` — audit detected the field mismatch and surfaced the check identifier.
- `1` — audit passed (silent regression) or failed without naming the check / field / file.

### Surface

`exit-code` — bash invokes the audit script and inspects exit + stderr.
