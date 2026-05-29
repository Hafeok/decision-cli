---
id: TC-204
title: JSON format serializes rows plus tally with stable shape
type: scenario
status: passing
validates:
  features:
  - FT-111
  adrs: []
observes:
- stdout
phase: 4
runner: cargo-test
runner-args: tc_204_json_format_stable_shape
runner-timeout: 30
last-run: 2026-05-29T09:21:58.939546097+00:00
last-run-duration: 0.6s
---

## Description

The JSON formatter is the machine-readable output operators (and
downstream tooling) parse. PAT-003's "pure formatter" discipline
means the shape is data-driven; this TC pins the contract so
downstream readers don't break on internal refactors.

## Acceptance Criteria

Given hand-constructed `rows` (one of each outcome variant —
Done, Stuck, HitMaxIter, Timeout, Error) and the derived
`tally`, call `format_json(&rows, &tally)` and assert:

1. Output parses as valid JSON.
2. Top-level keys are exactly `["rows", "tally"]`.
3. Each row object has keys `["feature_id", "outcome",
   "iterations", "elapsed_ms"]`.
4. The `outcome` field is a serde-tagged enum:
   - Done variant: `{"type": "done"}` (or the established adjacent
     pattern from existing oxi-events serializers — the test
     captures whatever convention ships).
   - Stuck: `{"type": "stuck", "reason": "..."}`.
   - Timeout: `{"type": "timeout", "after_secs": N}`.
5. The `tally` object has keys
   `["done", "stuck", "max_iter", "timeout", "error"]`.

The test snapshots the JSON against a fixture file
(`tests/fixtures/sweep-json.txt`) so the contract is reviewable
in PRs.