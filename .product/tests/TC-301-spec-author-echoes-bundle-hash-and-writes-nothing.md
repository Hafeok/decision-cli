---
id: TC-301
title: spec-author echoes bundle_hash and writes nothing beyond stdout
type: scenario
status: failing
validates:
  features:
  - FT-129
  adrs:
  - ADR-073
phase: 1
runner: pytest
runner-args: workers/spec-author/tests/test_bundle_hash_echo.py
runner-timeout: 60
observes:
- exit-code
- stdout
- disk-state
last-run: 2026-06-04T12:08:02.706715821+00:00
last-run-duration: 0.3s
failure-message: "ERROR: file or directory not found: workers/spec-author/tests/test_bundle_hash_echo.py\n\n"
---

## Purpose

Validates FT-129 (spec-author worker) against ADR-073's bundle-hash echo and stateless-worker invariants. Mirrors TC-289 for tc-author. Workers must echo the input `bundle_hash` so the harness can chain provenance, and must NEVER write to disk — bundle in, JSON to stdout, nothing else.

## Acceptance

- Parsed stdout `SpecProposal.bundle_hash` equals the input bundle's `bundle_hash`, byte for byte.
- No file is created, modified, or deleted in CWD during worker execution (directory snapshot diff is empty).
- No file is created, modified, or deleted under the test's tmp dir namespace.
- The worker exits with status code 0.

## Inputs

Synthetic bundle JSON written to a `tmp_path` fixture with a known `bundle_hash` (e.g. `"sha256:def456..."`). The Anthropic client is monkeypatched to return any well-formed `SpecProposal`. The test snapshots CWD and tmp before and after the `python -m spec_author <bundle-path>` invocation.

## Out of scope

- Proposal-payload correctness (covered by TC-298, TC-299, TC-300).
- Stderr logging discipline (logs are allowed; only stdout JSON and disk state are observed).