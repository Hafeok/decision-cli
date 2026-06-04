---
id: TC-357
title: add-artifact-type coherence audit rejects fixtures containing python files
type: scenario
status: passing
validates:
  features:
  - FT-141
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-357-cluster-audit-artifact-type-no-python.sh
runner-timeout: 60
observes:
- exit-code
- stderr
last-run: 2026-06-04T15:47:45.339341071+00:00
last-run-duration: 0.0s
---

## Context

Scenario TC for [FT-141](FT-141) (TaskType `add-artifact-type`). Exercises the **no-python-files** check (audit check 6 in FT-141 §Outputs) — the misclassification firewall against worker TaskTypes (`add-judge-worker`, `add-author-worker`). Those clusters emit `.py` files (pydantic IO models, agent loops, unit tests in pytest); this cluster emits ONLY Rust + Turtle. If a `.py` file appears in the cell outputs, the wrong TaskType has been dispatched and the cluster must not commit.

## Setup

A synthetic seven-file fixture committed under `tests/fixtures/cluster-audit-add-artifact-type/negative-python-leak/`. The fixture is identical to TC-355's positive case PLUS an extra file:

- `agent_loop.py` (stray, simulating misclassification) — minimal stub mimicking the shape of `workers/tc-author/src/tc_author/agent/loop.py`.

Every other file (`foo.rs`, `foo.shacl.ttl`, `foo_vocab.rs`, `parser.rs`, `emitter.rs`, `tests.rs`) is the TC-355 positive case — checks 1–5 would all pass if the audit only looked at those.

## Steps

1. `bash scripts/checks/tc-357-cluster-audit-artifact-type-no-python.sh` invokes the audit script with the seven fixture file paths.

## Expected outcome

- Audit script exits non-zero (1 = audit failure, OR 2 = unrunnable; both are acceptable misclassification signals — the cluster MUST NOT commit either way).
- Stderr contains the failing check identifier: `no-python-files`.
- Stderr names the offending file: `agent_loop.py`.

## Pass / fail

- Pass: shell wrapper script asserts non-zero exit + presence of `no-python-files` and `agent_loop.py` in audit stderr.
- Fail: audit exits 0 (misclassification firewall is breached — the wrong TaskType could ship Python code through this cluster's commit path), OR stderr omits the check identifier (caught but operator gets no actionable hint that the symptom is misclassification rather than legitimate cluster failure).

## Why this scenario

Per ADR-080 §Consequences, "misclassification has an explicit escape" — the broad worker is the documented `not_confident → broad worker` branch. But the classifier's confidence signal is soft (operator-declared `task_type:` in front-matter), so a typo or a copy-pasted feature_spec header can route a worker task through this cluster's cells. Without check 6, the cluster dispatcher would happily emit a Rust struct + SHACL + parser + emitter + tests for what should have been a Python worker, the audit's other 5 checks would pass on the rust artefacts, and the commit would land — producing a half-implemented worker feature that fails much later in `dec drive` for unrelated reasons. Check 6 makes the misclassification audible at the cluster boundary where the operator can re-classify.

This TC is also FT-141's explicit observability contract per its `domains-acknowledged: observability` reason: the `no-python-files` check identifier surfaces in the `ClusterAuditFailed { check, detail }` outcome verbatim, and this TC asserts that observability surface.