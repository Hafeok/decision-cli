---
id: TC-098
title: dec verify env new rejects invalid --fixture-source paths
type: scenario
status: passing
validates:
  features:
  - FT-053
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-098-env-new-fixture-source-rejects.sh
runner-timeout: 120
last-run: 2026-05-22T13:02:35.600220671+00:00
last-run-duration: 0.2s
---

## Purpose

FT-053 path validation rejects `--fixture-source` values that are not safe to materialise:

- **Absolute paths** would break reproducibility across hosts (ADR-032 §Rejected alternatives).
- **`..` segments** would let the fixture escape the repo root.
- **Non-existent paths** would surface as runtime errors deep inside the executor — better fail at authoring time.
- **Paths pointing at a regular file** (not a directory) can't be materialised as a tree.

## Given

- A working directory with `dec init --template engineering-development` completed.

## When

The four invocations below are each run in a fresh subshell with the same env-new arguments except for `--fixture-source`:

| Variant | `--fixture-source` value |
|---|---|
| absolute | `/etc` |
| parent-dir | `tests/../etc` |
| missing | `tests/fixtures/__does_not_exist__` |
| file | `Cargo.toml` |

## Then

Each invocation exits non-zero. The stderr contains the substring `fixture_source` and an error class hint:

| Variant | Required substring |
|---|---|
| absolute | `repo-relative` |
| parent-dir | `..` |
| missing | `does not exist` |
| file | `not a directory` |

No `.dec/verify/env/*.ttl` is written for any of the four variants (`ls .dec/verify/env/` after each invocation shows no new files).

## Notes

- The SHACL shape's `min-length 1` separately rejects empty / whitespace-only fixture_source values, but that path is unreachable from the CLI surface (clap's `Option<String>` collapses `""` to `Some("")` only with explicit empty quoting, which the validate.rs layer trims and rejects).