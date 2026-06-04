---
id: TC-361
title: 'add-cli-subcommand: coherence audit FAILS with check=integration_test_path when no file under crates/decision-cli/tests/ is emitted'
type: scenario
status: passing
validates:
  features:
  - FT-142
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-361-cluster-audit-cli-subcommand-no-integration-test.sh
runner-timeout: 60
observes:
- exit-code
- stderr
last-run: 2026-06-04T15:47:45.847784661+00:00
last-run-duration: 0.0s
---

## Description

Negative scenario for the `add-cli-subcommand` coherence audit's `integration_test_path` check — the **structural discriminator** that prevents misclassification with the `add-artifact-type` task type (whose tests live under `crates/decision-cli/src/.../tests.rs` as unit tests) and the `add-judge-worker` / `add-author-worker` task types (whose tests live under `workers/<name>/tests/`).

## Acceptance criteria

1. Fixture directory contains five cell outputs (a perfectly-internally-consistent `add-cli-subcommand` cluster) **except** that no file is emitted under `crates/decision-cli/tests/`. Instead, a unit test is placed inline under `crates/decision-cli/src/features/example_verb/tests.rs` (mimicking what the `add-artifact-type` cluster would have emitted).
2. The other five checks all hold on this fixture: `fields_used`, `flags_tested` (via the unit test that does cover every flag — the regex search is not path-restricted on its own), `flags_documented`, `wiring_imports_both`, `mcp_calls_handler` all pass; only the path-glob check would fail.
3. Running `bash scripts/checks/tc-361-cluster-audit-cli-subcommand-no-integration-test.sh` constructs the fixture, invokes `scripts/checks/cluster-audit-add-cli-subcommand.py` with the fixture paths and `--surfaces-via-mcp`, and asserts:
   - Exit code is **1** (audit fail), not 0.
   - Stderr contains the literal string `check=integration_test_path`.
   - Stderr message mentions the expected glob `crates/decision-cli/tests/` so the operator sees what the cluster needed to emit.
4. Re-running the audit with the same fixture but with the unit test relocated to `crates/decision-cli/tests/example_verb.rs` (so the path glob is satisfied) exits 0 — confirming the failure is the path glob, not anything else.

## Runner

`bash scripts/checks/tc-361-cluster-audit-cli-subcommand-no-integration-test.sh` — exit 0 = audit correctly failed with check=integration_test_path; exit 1 = audit failed with a different check OR did not fail; exit 2 = harness unrunnable.

## What this guards

The structural discriminator against TaskType misclassification. The classifier dispatches `add-cli-subcommand` based on the operator-declared `task_type:` value — if the operator mistakenly marks a feature that's actually adding a worker (or an artifact type) as `add-cli-subcommand`, the cluster will run, the LLM cells will emit, and the artifacts will land in the wrong directories. Without TC-361 the audit could miss this entire class of confidently-wrong-cluster failure mode (per ADR-080 §Decision §2: *"Misclassification dispatches a confidently-wrong cluster"*). With TC-361, the `integration_test_path` glob acts as the cheap, structural cross-check that the dispatcher landed the cluster's outputs in the directories that match this TaskType's identity, not another.