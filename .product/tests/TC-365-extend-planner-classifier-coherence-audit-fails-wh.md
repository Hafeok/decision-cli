---
id: TC-365
title: extend-planner-classifier coherence audit FAILS when state_hash_update does not fold in new signal (FT-138 TC-349 silent-regression guard generalised)
type: scenario
status: unimplemented
validates:
  features:
  - FT-143
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-365-cluster-audit-planner-classifier-state-hash-missing.sh
runner-timeout: 60
observes:
- exit-code
- stderr
---

## Context

Scenario TC for [FT-143](FT-143). Asserts the coherence audit catches the **state-hash silent regression** failure mode (audit check 3 in FT-143 §Behaviour §Phase 2) and surfaces the failing check identifier verbatim on stderr.

This is the **generalisation** of [FT-138](FT-138)'s `TC-349` (the silent-regression guard for the open-implementer-feedback state-hash update). FT-138 caught this property feature-locally; TC-365 lifts it to a cluster-wide invariant the audit enforces for every future `extend-planner-classifier` drive. An implementer (LLM or human) who emits `classifier_row` + `unit_tests` but forgets to fold the new signal into `state_hash_update` would pass the precedence/positive/negative unit tests AND ship a broken cycle detector that false-positives across legitimate lifecycle transitions in production. The audit must catch this at cluster time, before the worktree commits.

## Setup

- The audit script `scripts/checks/cluster-audit-extend-planner-classifier.py` is on disk and executable.
- A fixture directory under `tests/fixtures/cluster-audit-extend-planner-classifier/state-hash-missing/` containing 6 cell outputs identical to TC-363's positive fixture **except**:
  - `state_hash_update.rs`: emits the cell body and a `classify_and_hash` fragment, but the new signal's name (`has_open_implementer_feedback_for_feature` or its corresponding `let` binding) does NOT appear in the hasher's write region. Possibly the cell only emits the trailing logic and forgets the `hasher.update(&[<signal> as u8])` call.
- A bash runner under `tests/scripts/tc-365-cluster-audit-planner-classifier-state-hash-missing.sh` that invokes the audit and asserts the failure.

## Steps

1. Execute `tests/scripts/tc-365-cluster-audit-planner-classifier-state-hash-missing.sh`.
2. The script invokes `python3 scripts/checks/cluster-audit-extend-planner-classifier.py <6 cell paths>`.
3. Capture exit code and stderr.

## Expected outcome

- Exit code: `1` (audit failure).
- Stderr contains the check-3 identifier verbatim — e.g. `check=state_hash_includes_new_signal` or the exact string the audit script emits for that check, plus the missing signal name.

## Pass / fail

- Pass: bash script exits 0 (audit exited 1 with the check-3 stderr marker present).
- Fail: audit exited 0 (false negative — the silent regression slipped through), OR exited 1 without the check-3 identifier (audit failed but for the wrong reason).

## Why this matters

[FT-138](FT-138)'s author hand-wrote TC-349 because the property is non-obvious — a missing `hasher.update(...)` for the new signal does not cause any unit test to fail (the classifier still picks the right action). The breakage only appears in live `dec drive` loops, when the cycle detector mistakes legitimate `produced → addressed` transitions for cycles and dispatches a stale plan. TC-365 generalises this audit obligation to the cluster pattern itself — every `extend-planner-classifier` drive going forward gets the same protection without each feature_spec author needing to remember to write a per-feature TC-349-style test. This is the unique payoff of typed cell decomposition vs the monolith: a load-bearing safety property gets enforced *once* in the audit and inherits to every consumer.
