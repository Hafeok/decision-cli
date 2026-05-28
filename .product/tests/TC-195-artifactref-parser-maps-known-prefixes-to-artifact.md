---
id: TC-195
title: ArtifactRef parser maps known prefixes to ArtifactKind and rejects malformed input
type: exit-criteria
status: failing
validates:
  features:
  - FT-110
  adrs: []
phase: 3
runner: cargo-test
runner-args: tc_195_artifact_parser_prefix_map
runner-timeout: 60
last-run: 2026-05-28T08:49:19.011623470+00:00
last-run-duration: 0.4s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

`core::drive::artifact::ArtifactRef::parse` correctly identifies each supported prefix and returns the matching `ArtifactKind`. Unknown / malformed input returns `Err::InvalidArgument` whose message lists the supported prefixes.

## Scenarios

### Happy paths

| Input | Expected `ArtifactKind` |
|---|---|
| `FT-019` | `Feature` |
| `TC-027` | `TestCriterion` |
| `VG-100` | `VerificationGraph` |
| `ENV-002` | `Environment` |
| `ENV-001-ephemeral-cli` | `Environment` |
| `ADR-066` | `Adr` |

### Rejection paths

| Input | Why |
|---|---|
| `FT019` (no hyphen) | malformed |
| `XX-000` | unknown prefix |
| empty string | malformed |
| `feature-019` (lowercase) | malformed |

The error variant must be `Err::InvalidArgument`, and its `detail` must mention at least one supported prefix so the operator can self-correct.