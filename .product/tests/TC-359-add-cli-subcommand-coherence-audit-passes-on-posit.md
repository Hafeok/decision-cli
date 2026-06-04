---
id: TC-359
title: 'add-cli-subcommand: coherence audit passes on positive fixture (all six checks green)'
type: scenario
status: passing
validates:
  features:
  - FT-142
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/tc-359-cluster-audit-cli-subcommand-positive.sh
runner-timeout: 60
observes:
- exit-code
- stderr
last-run: 2026-06-04T15:47:45.847784661+00:00
last-run-duration: 0.0s
---

## Description

Positive scenario for the `add-cli-subcommand` coherence audit. Validates that when the six (or five, with MCP omitted) cell outputs are internally consistent, `scripts/checks/cluster-audit-add-cli-subcommand.py` exits 0 with no failing-check stderr.

## Acceptance criteria

1. Fixture directory contains the six cell outputs for a synthetic `dec example-verb` subcommand:
   - `crates/decision-cli/src/cli/example_verb.rs` with a `pub struct Args { pub name: String, pub verbose: bool, pub output: PathBuf }` and `///` doc comments on every field.
   - `crates/decision-cli/src/features/example_verb/mod.rs` with `pub fn run(args: Args, ctx: &Context) -> Result<ExitCode>` whose body references `args.name`, `args.verbose`, and `args.output`.
   - `crates/decision-cli/src/main.rs` patch importing `crate::cli::example_verb::Args` AND `crate::features::example_verb::run`.
   - `crates/decision-cli/src/core/mcp/example_verb.rs` importing and invoking `features::example_verb::run`.
   - `crates/decision-cli/tests/example_verb.rs` with at least one `#[test]` for each of `--name`, `--verbose`, `--output`.
   - Help doc string output listing `--name`, `--verbose`, `--output` verbatim.
2. Running `bash scripts/checks/tc-359-cluster-audit-cli-subcommand-positive.sh` constructs the fixture, invokes `scripts/checks/cluster-audit-add-cli-subcommand.py` with the fixture paths and `--surfaces-via-mcp`, and asserts exit 0.
3. Stderr is empty (or contains only INFO-level diagnostics — no `check=` lines).
4. Each of the six checks (`fields_used`, `flags_tested`, `flags_documented`, `wiring_imports_both`, `mcp_calls_handler`, `integration_test_path`) is reported as `OK` on stdout in a structured form the operator can grep.

## Runner

`bash scripts/checks/tc-359-cluster-audit-cli-subcommand-positive.sh` — exit 0 = audit passes on positive fixture; exit 1 = audit (incorrectly) failed; exit 2 = harness unrunnable.

## What this guards

Without a positive scenario, a perpetually-failing audit (overly-strict regex, broken fixture) would look indistinguishable from "audit catches real bugs" — every cluster fails for the wrong reason. TC-359 establishes the floor: under known-good cell outputs, the audit MUST exit 0.