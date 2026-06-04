---
id: TC-374
title: add-judge-worker coherence audit passes on a positive fixture with cell agreement
type: scenario
status: unimplemented
validates:
  features:
  - FT-139
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-374-cluster-audit-judge-worker-positive.sh
runner-timeout: 60
observes:
- exit-code
---

## Acceptance criteria

Positive-case test for the `add-judge-worker` coherence audit. Verifies that [FT-139](FT-139) §Phase 4's `scripts/checks/cluster-audit-add-judge-worker.py` accepts a cell-output set where all five cells agree on the input contract. Pair with TC-372 (the negative-case teeth test).

### Conditions

Bash runner script against a synthetic-positive cell-output fixture under `tests/fixtures/cluster-audit-add-judge-worker/positive/`:

- `pydantic_io_models.py` declares `class JudgeInput { feature_id: str; proposed_artifact: str }` and `class JudgeOutput { verdict: str; reasoning: str }`.
- `agent_loop.py` references only `payload.feature_id` and `payload.proposed_artifact` — both present on the model.
- `system_prompt.md` Jinja template references only `{{feature_id}}` and `{{proposed_artifact}}` — both present.
- `capability_binding.nq` carries `endpoint = openai/scaleway-llama-...` and `model_id` matching one of the recognised provider prefixes.
- `unit_tests.py` constructs a fixture `JudgeInput` and a fixture `JudgeOutput` — validates each against its model.

Setup:
- Invoke `python3 scripts/checks/cluster-audit-add-judge-worker.py <fixture-dir>`.

Assertions:
- Exit code is `0` (audit pass).
- stderr is empty (or contains only INFO-level lines, not WARN or FAIL).
- stdout contains a one-line summary `PASS add-judge-worker (5 checks passed)`.

### Why this matters

A coherence audit that never accepts a valid cluster is worse than no audit — every dispatch becomes false-positive blocked. The positive case is the audit's basic competence test. Pair with TC-372: together they prove the audit has teeth (negative fails) and discriminates (positive passes).

### Exit codes

- `0` — audit accepted the consistent fixture.
- `1` — audit incorrectly rejected, or summary line is malformed.

### Surface

`exit-code` — bash invokes the audit script and inspects exit + stdout + stderr.
