---
id: TC-259
title: dec drive ship accepts FT-XXX as first positional without requiring a goal positional
type: invariant
status: failing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-259-drive-ship-cli-shape.sh
runner-timeout: 30
observes:
- stdout
- exit-code
last-run: 2026-05-30T16:07:54.658458112+00:00
last-run-duration: 0.0s
failure-message: "REGRESSION: `dec drive ship` requires <GOAL> as first positional.\n            Expected: `Usage: dec drive ship [OPTIONS] <ARTIFACT>` (or [ARTIFACT]).\n            Got:\n    Run a goal-driven dispatch loop (FT-110, FT-111)\n    \n    Usage: dec drive ship [OPTIONS] <GOAL> [ARTIFACT]\n    \n    Arguments:\n      <GOAL>      Goal: `ship`, `verify`, `accept`, `cover`, or `approve`\n      [ARTIFACT]  Artifact short id (e.g. `FT-019`, `TC-027`). Mutually exclusive with --all\n    \n    Options:\n          --all\n"
---

## Description

Regression test pinning the public `dec drive ship` CLI shape.

Before FT-113 the invocation was `dec drive ship FT-XXX
[--bench BNCH-NNN]`. FT-113's drive-show CLI re-org split
`dec drive` into subcommands but kept the original goal
positional on `ship`, so the post-FT-113 binary required
`dec drive ship ship FT-XXX --bench BNCH-NNN` (the
operator must type `ship` twice). Every existing user
script, MCP integration, and the harness's own dispatcher
broke at the same moment.

This TC ensures the `ship` subcommand takes the artifact id
as its first non-option positional, with no extra `<GOAL>`
positional in front of it.

## Acceptance Criteria

Bash test asserts both the help text and a real invocation:

1. `dec drive ship --help` lists `<ARTIFACT>` (or
   `[ARTIFACT]`) as the first positional. If the usage
   line shows `<GOAL>` first, the test fails with a clear
   regression message.

2. `dec drive ship FT-114 --bench BNCH-002` (FT-114 ships
   complete already, so the planner reaches Done in iter 0
   without dispatching workers) exits 0 AND the combined
   stdout/stderr does NOT contain the substring
   `artifact required`.

The test runs against the installed `dec` on `$PATH`
(via `cargo install --path crates/decision-cli`); both
assertions are deterministic and complete in a couple of
seconds.