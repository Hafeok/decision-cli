---
id: TC-289
title: tc-author echoes bundle_hash and writes nothing beyond stdout
type: scenario
status: passing
validates:
  features:
  - FT-126
  adrs:
  - ADR-073
phase: 1
runner: pytest
runner-args: workers/tc-author/tests/test_bundle_hash_echo.py
runner-timeout: 60
observes:
- exit-code
- stdout
- disk-state
last-run: 2026-06-04T09:19:14.427479625+00:00
last-run-duration: 0.4s
---

## Purpose

Validates FT-126 (tc-author worker) against the bundle-hash echo and stateless-worker contract of ADR-073. Workers must echo the input `bundle_hash` so the harness can chain provenance, and must NEVER touch the filesystem — bundle in, JSON out. Mirrors TC-078 for FT-048's implementer worker.

## Acceptance

- Parsed stdout `TcProposal.bundle_hash` equals the `bundle_hash` field of the input bundle, byte for byte.
- No file is created, modified, or deleted in the CWD during worker execution (verified via directory snapshot diff).
- No file is created, modified, or deleted under the system temp directory namespace owned by the worker.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON written to a `tmp_path` fixture with a known `bundle_hash` (e.g. `"sha256:abc123..."`). The Anthropic client is monkeypatched to return any well-formed `TcProposal`. The test snapshots CWD and tmp dir contents before invocation, runs `python -m tc_author <bundle-path>`, then snapshots again and asserts equality.

## Out of scope

- Correctness of the proposal payload itself (covered by TC-286 / TC-287 / TC-288).
- Stderr logging discipline (logs are allowed; only stdout JSON and disk state are observed).