---
id: TC-016
title: source_file_length_within_adr_013_limits
type: invariant
status: failing
validates:
  features: []
  adrs:
  - ADR-013
phase: 1
runner: bash
runner-args: scripts/checks/file-length.sh
runner-timeout: 60
last-run: 2026-05-19T12:13:19.277116267+00:00
last-run-duration: 0.0s
failure-message: ""
---

## Purpose

Mechanical enforcement of **ADR-013 Rule 1 — File Size Limit**. Asserts that
every first-party source file under `crates/*/src/` (Rust) and `workers/*/`
(Python, excluding `tests/`) is at most 400 lines (hard limit). Files in the
300–400 line band produce a warning (exit 2) — informative, non-blocking.
Files over 400 lines produce a hard failure (exit 1) — CI blocks the merge.

This TC has empty `validates.features` by design: per ADR-014, code-quality
rules are cross-cutting, validated against every feature implicitly via
`product verify --platform`.

## Given

- A working copy of the decision-cli repository checked out at any commit.
- `bash`, `awk`, `wc`, and `find` available on `PATH` (the only dependencies).

## When

```bash
scripts/checks/file-length.sh
```

## Then

1. Exit 0 if every first-party `*.rs` under `crates/*/src/` and every
   first-party `*.py` under `workers/*/` (excluding `tests/`) is at most
   `FILE_LENGTH_WARN` lines (default 300).
2. Exit 2 if at least one such file is in the 301–400 line band and none
   exceeds the hard limit. Diagnostic lines on stdout name each warning file.
3. Exit 1 if at least one such file exceeds `FILE_LENGTH_HARD` lines
   (default 400). Diagnostic lines on stdout name each offending file with
   its line count.

## Notes

- Thresholds may be overridden via `FILE_LENGTH_HARD` and `FILE_LENGTH_WARN`
  environment variables.
- TC-CQ-001 in the workspace narrative; TC-016 in the graph.
- This is the *first* mechanical rule shipped under the ADR-014 convention
  (FT-015). Subsequent rules (function length, module structure,
  single-responsibility doc comments) land as part of FT-014 and each adds
  its own TC pointing to the same parent ADR (ADR-013).