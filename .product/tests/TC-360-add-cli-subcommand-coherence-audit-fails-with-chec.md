---
id: TC-360
title: 'add-cli-subcommand: coherence audit FAILS with check=flags_tested when integration test omits an advertised flag'
type: scenario
status: passing
validates:
  features:
  - FT-142
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-360-cluster-audit-cli-subcommand-missing-flag-test.sh
runner-timeout: 60
observes:
- exit-code
- stderr
last-run: 2026-06-04T15:47:45.847784661+00:00
last-run-duration: 0.0s
---

## Description

Negative scenario for the `add-cli-subcommand` coherence audit's `flags_tested` check. Validates that when the integration test omits one of the flags advertised in the clap args module, the audit fails loudly with the specific check identifier on stderr.

## Acceptance criteria

1. Fixture directory contains the six cell outputs from TC-359's positive fixture **with one mutation**: the integration test file (`crates/decision-cli/tests/example_verb.rs`) is edited to remove every reference to `--output` (the flag still exists on the clap args struct and is referenced in the handler, but no test exercises it).
2. Running `bash scripts/checks/tc-360-cluster-audit-cli-subcommand-missing-flag-test.sh` constructs the mutated fixture, invokes `scripts/checks/cluster-audit-add-cli-subcommand.py` with the fixture paths and `--surfaces-via-mcp`, and asserts:
   - Exit code is **1** (audit fail), not 0.
   - Stderr contains the literal string `check=flags_tested` AND the literal string `output` (the missing flag name).
   - Stderr does NOT contain `check=fields_used`, `check=flags_documented`, `check=wiring_imports_both`, `check=mcp_calls_handler`, or `check=integration_test_path` (these all still hold — the mutation is surgical).
3. Re-running the audit on the **unmutated** TC-359 fixture (same script's setup, mutation reverted) exits 0 — confirming the failure is caused by the mutation, not the harness.

## Runner

`bash scripts/checks/tc-360-cluster-audit-cli-subcommand-missing-flag-test.sh` — exit 0 = audit correctly failed with check=flags_tested; exit 1 = audit failed but with a different check OR did not fail; exit 2 = harness unrunnable.

## What this guards

The audit's "teeth" property — without this negative case, the audit could be a no-op that always exits 0 and still satisfy TC-359. TC-360 forces a real mutation that the audit MUST catch, and pins the check identifier verbatim so the operator can map `ClusterAuditFailed { check: "flags_tested" }` back to the witnessed regression class. This is the ADR-080 §Consequences point: *"the audit catches a divergence the broad worker would have caught for free in a shared context."*