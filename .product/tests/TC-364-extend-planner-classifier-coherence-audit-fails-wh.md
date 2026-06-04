---
id: TC-364
title: extend-planner-classifier coherence audit FAILS when inspector_trait_method and inspector_production_impl return types disagree
type: scenario
status: passing
validates:
  features:
  - FT-143
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-364-cluster-audit-planner-classifier-type-mismatch.sh
runner-timeout: 60
observes:
- exit-code
- stderr
last-run: 2026-06-04T15:47:46.291419308+00:00
last-run-duration: 0.0s
---

## Context

Scenario TC for [FT-143](FT-143). Asserts the coherence audit script catches the **return-type triple disagreement** failure mode (audit check 1 in FT-143 §Behaviour §Phase 2) and surfaces the failing check identifier verbatim on stderr per FT-143's "audit failure is loud and specific" invariant.

This is one of the two negative-teeth TCs (the other is TC-365). Together they prove the audit catches divergences the broad worker would otherwise have caught implicitly via shared context — the load-bearing property of the entire ADR-080 decomposition per the SDLC doc's analysis.

## Setup

- The audit script `scripts/checks/cluster-audit-extend-planner-classifier.py` is on disk and executable.
- A fixture directory under `tests/fixtures/cluster-audit-extend-planner-classifier/type-mismatch/` containing 6 cell outputs identical to TC-363's positive fixture **except**:
  - `inspector_trait_method.rs`: declares the method with return type `Result<bool, InspectError>`.
  - `inspector_production_impl.rs`: production override declares return type `Result<u32, InspectError>` (deliberate divergence).
  - `inspector_default_impl.rs` and remaining cells unchanged from positive fixture.
- A bash runner under `tests/scripts/tc-364-cluster-audit-planner-classifier-type-mismatch.sh` that invokes the audit and asserts the failure.

## Steps

1. Execute `tests/scripts/tc-364-cluster-audit-planner-classifier-type-mismatch.sh`.
2. The script invokes `python3 scripts/checks/cluster-audit-extend-planner-classifier.py <6 cell paths>`.
3. Capture exit code and stderr.

## Expected outcome

- Exit code: `1` (audit failure).
- Stderr contains the check-1 identifier verbatim — e.g. `check=return_type_triple_agreement` or the exact string the audit script emits for that check, plus a diff showing `bool` vs `u32`.

## Pass / fail

- Pass: bash script exits 0 (which means the audit script exited 1 with the expected stderr marker — the runner wraps the audit invocation and inverts the success condition for negative tests).
- Fail: audit script exited 0 (false negative — divergence was not caught), OR exited 1 without the check-1 identifier (audit failed but for the wrong reason — could mask the actual bug).

## Why this matters

The return-type triple agreement is the most basic safety property of the cluster: trait signature, default impl, and production impl must all return the same `Result<T, InspectError>`. If they diverge, the code does not compile — the broad worker would have noticed because all three live in its single context. The audit must catch this mechanically because the cell decomposition loses the shared context. Without TC-364 passing, the decomposition is strictly worse than the monolith per ADR-080 §Decision §3 and the SDLC doc's load-bearing-audit principle.