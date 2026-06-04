---
id: TC-363
title: extend-planner-classifier coherence audit passes on positive fixture (all 6 cells emit row fitting between named adjacent rows)
type: scenario
status: passing
validates:
  features:
  - FT-143
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-363-cluster-audit-planner-classifier-positive.sh
runner-timeout: 60
observes:
- exit-code
- stderr
last-run: 2026-06-04T15:47:46.291419308+00:00
last-run-duration: 0.0s
---

## Context

Scenario TC for [FT-143](FT-143) (TaskType `extend-planner-classifier`). Asserts the coherence audit script `scripts/checks/cluster-audit-extend-planner-classifier.py` returns exit 0 on a synthetic positive fixture where all 6 cells emit fragments that satisfy all 6 audit checks listed in FT-143 §Behaviour §Phase 2.

This is the load-bearing positive of the cluster's coherence audit — if the audit cannot accept a well-formed cluster output, the TaskType is unusable.

## Setup

- The audit script `scripts/checks/cluster-audit-extend-planner-classifier.py` is on disk and executable.
- A fixture directory under `tests/fixtures/cluster-audit-extend-planner-classifier/positive/` containing one file per cell, hand-authored to satisfy every check:
  - `inspector_trait_method.rs`: declares `fn has_open_implementer_feedback_for_feature(&self, feature_id: &str) -> Result<bool, InspectError>` (signature only, on the trait).
  - `inspector_default_impl.rs`: default trait body returning `Ok(false)`.
  - `inspector_production_impl.rs`: override returning `Result<bool, InspectError>`, body uses `resolve_feature_tcs_short` + SPARQL `ASK`.
  - `classifier_row.rs`: includes `self.inspector.has_open_implementer_feedback_for_feature(feature_id)?` AND a positional comment `// FT-138 / ADR-079: above vgs_cover_present_state_for_feature, below tcs_linked_state_for_feature`.
  - `state_hash_update.rs`: a fragment of `classify_and_hash` whose hasher-write region references the new boolean by name (`has_open_implementer_feedback_for_feature`).
  - `unit_tests.rs`: at least 4 `#[test]` functions with names matching `precedence_*`, `positive_*`, `negative_*`, `state_hash_*`.
- A merged planner-file snapshot under the fixture demonstrating the row appears between the two named adjacent rows (for check 5's text-order assertion).
- A bash runner under `tests/scripts/tc-363-cluster-audit-planner-classifier-positive.sh` that invokes the audit with the 6 fixture paths.

## Steps

1. Execute `tests/scripts/tc-363-cluster-audit-planner-classifier-positive.sh`.
2. The script invokes `python3 scripts/checks/cluster-audit-extend-planner-classifier.py <6 cell paths>`.
3. Capture exit code and stderr.

## Expected outcome

- Exit code: `0` (audit pass).
- Stderr: empty or informational only — no `ClusterAuditFailed` markers.

## Pass / fail

- Pass: bash script exits 0.
- Fail: bash script exits non-zero (audit incorrectly rejected a well-formed cluster).

## Why this matters

The audit is the load-bearing contract enforcement for the TaskType. A false positive (audit rejects valid output) means future drives can never ship through this cluster — the broad-worker fallback absorbs everything and the TaskType decomposition is functionally inert. TC-363 is the gate: the audit must accept the hand-authored canonical shape derived from the FT-138 witnessed example before any LLM-driven cell output is dispatched.