---
id: TC-199
title: Verdict aggregator demotes Rejected to AmendmentRequired when an evidence-bearing step exits with a graph-fault exit code (2, 126, 127)
type: exit-criteria
status: failing
validates:
  features:
  - FT-110
  adrs: []
phase: 3
runner: cargo-test
runner-args: tc_199_verdict_demotes_on_graph_fault_exit
runner-timeout: 60
last-run: 2026-05-28T08:49:19.011623470+00:00
last-run-duration: 0.4s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

`single_graph_verdict_with_exit_codes` demotes the verdict from `Rejected` to `AmendmentRequired` when a failing evidence-bearing step's exit code is in `GRAPH_FAULT_EXIT_CODES` (currently `{2, 126, 127}`). Exit codes outside that set (e.g. `1`, `101`) keep the original `Rejected` classification. The rationale string names the offending exit code so the operator can diagnose without spelunking through the dump.

## Scenarios

### Graph-fault demotion table

For each `(exit_code, expected_verdict)` row, build a single-step graph with evidence-bearing `provides_evidence_for = ["TC-T199a"]` and a `Fail` outcome:

| exit_code | expected verdict |
|---|---|
| `Some(2)` | `AmendmentRequired` |
| `Some(126)` | `AmendmentRequired` |
| `Some(127)` | `AmendmentRequired` |
| `Some(1)` | `Rejected` |
| `Some(101)` | `Rejected` |
| `None` | `Rejected` |

For `AmendmentRequired` rows, the rationale must mention the exit code AND the phrase "graph-design fault".

### Non-evidence steps unchanged

A failing non-evidence-bearing step (`provides_evidence_for: []`) returns `AmendmentRequired` regardless of exit code (existing FT-097 behaviour, preserved). The new exit-code logic only activates on evidence-bearing rows.

### Back-compat wrapper

`single_graph_verdict(traces, evidence)` (two-arg wrapper) returns the legacy verdict for evidence-bearing fails — `Rejected` regardless of exit code, because the wrapper passes empty exit_codes.