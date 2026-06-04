---
id: TC-352
title: add-author-worker coherence audit fails when Output schema has verdict instead of body_markdown
type: scenario
status: unimplemented
validates:
  features:
  - FT-140
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-352-cluster-audit-author-negative.sh
runner-timeout: 60
---

## Context

Negative coherence-audit TC for [FT-140](FT-140) — THE DISCRIMINATOR test that proves the `add-author-worker` audit catches the misclassification a single broad-worker context would have caught implicitly. Per [ADR-080](ADR-080): *"the load-bearing audit of the whole pattern — worth prototyping first. If it is weaker than what a single context gave for free, the decomposition is worse than the monolith."*

This test is the safety-property witness for the author-vs-judge distinction: if a cluster authored under `add-author-worker` accidentally emits a judge-shaped Output (`verdict: str` instead of `body_markdown: str`), the audit MUST fail with a specific, operator-actionable message — not silently pass.

## Setup

- A fixture directory under `tests/fixtures/cluster-audit-add-author-worker/negative-verdict/` identical to the positive fixture EXCEPT:
  - `pydantic_io_models/models.py`'s `Output` class declares `verdict: str` (and lacks `body_markdown`), as if the cluster had been misclassified — a judge worker's contract dispatched into an author slot.
- All other cells (loop, prompt, fixtures, tests) are internally consistent with each other and with the (judge-shaped) Output. The audit's only signal is the Output-schema discriminator check.

## Steps

1. Run `scripts/checks/cluster-audit-add-author-worker.py` against the negative fixture directory.
2. Capture exit code and stderr.

## Expected outcome

- Exit code 1 (audit failure, not unrunnable).
- Stderr contains a `FAIL output_is_draft_not_verdict` line.
- The `detail` portion of that FAIL line contains the canonical hint string: `output is a verdict, not a draft — did you mean add-judge-worker?` (or a substring match on `did you mean add-judge-worker`).
- Other checks may PASS or FAIL depending on cascading effects of the Output rename; the discriminator check failing with the canonical hint is the binding assertion.

## Pass / fail

- Pass: bash runner exits 0 because the script wrapper asserts the audit failed with exit 1 AND stderr matched the canonical hint substring.
- Fail: the audit script unexpectedly exits 0 (audit has no teeth) OR exits 2 (unrunnable — fixture broken) OR exits 1 without the canonical hint (audit fails for the wrong reason).

## Why this is the load-bearing TC

This is the SDLC doc's *"audit catches a divergence the broad worker would have caught for free in a shared context"* — the cluster's safety property reduced to a single binary signal. If this test passes, the cluster pattern proves it has teeth for author workers and the decomposition is worth its cost. If it fails, the author cluster reduces to a confidently-wrong worker generator and the slice is not done.
