---
id: TC-091
title: Python verifier worker returns a structured VerificationVerdict over a bundled run
type: exit-criteria
status: failing
validates:
  features:
  - FT-023
  adrs:
  - ADR-018
phase: 2
runner: bash
runner-args: workers/verifier/tests/run-verifier-tc.sh
runner-timeout: 180
last-run: 2026-05-21T13:31:41.447481757+00:00
last-run-duration: 0.0s
failure-message: "bash: line 1: workers/verifier/tests/run-verifier-tc.sh: No such file or directory\n"
---

## Purpose

Exit criterion for [FT-023](FT-023): the Python verifier worker, invoked over a sample bundle (CodeChange + feature_spec + TCs + ADRs), returns a structured `VerificationVerdict` parseable by the slice-2 Rust reader.

## Given

A synthetic bundle JSON file with a representative `CodeChange`, the corresponding `feature_spec` body, two TCs, and one ADR. Anthropic SDK mocked or stubbed (no live network call).

## When

```bash
python -m verifier --bundle <path-to-bundle.json>
```

## Then

- Exit code is 0.
- stdout contains a single JSON document parseable as `VerificationVerdict`.
- The verdict's `kind` is one of `approved | amendment-required | rejected` ([ADR-018](ADR-018)).
- `cites` is non-empty when `kind != approved`.
- `bundle_hash` echoes the input bundle's hash.

## Notes

FT-023 is on a deprecation path per [ADR-028](ADR-028) — slice 3's graph executor supersedes it. This exit criterion documents the slice-2 contract for the verifier worker; once slice 3 lands, the role is reframed as a special-case step kind (`llm-judgment`) rather than a standalone worker.