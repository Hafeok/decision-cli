---
id: TC-078
title: verify-graph-author worker echoes bundle_hash for protocol integrity
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 2
runner: pytest
runner-args: workers/verify-graph-author/tests/test_tc_078_bundle_hash.py
runner-timeout: 120
last-run: 2026-05-23T16:10:08.067362560+00:00
last-run-duration: 0.4s
---

## Premise

A bundle with `bundle_hash = "abc123..."` is sent to the worker; the mocked Claude returns a `New` proposal but with `bundle_hash = "wrong-hash"` (simulating a stale or corrupted echo).

## Acceptance Criteria

- The worker's internal validation detects the hash mismatch.
- The worker exits with non-zero (5 per [FT-048](FT-048)'s error table) and a structured stderr message identifying the mismatch.
- No `GraphProposal` is written to stdout (or any output is marked as invalid).

## Notes

Bundle-hash echoing is the protocol integrity check between worker and harness. If the worker silently swallowed a wrong hash, the harness in [FT-049](FT-049) could not detect cross-bundle contamination.