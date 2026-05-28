---
id: TC-197
title: Driver respects max-iterations and surfaces a Stuck reason when a planner has no path forward
type: exit-criteria
status: failing
validates:
  features:
  - FT-110
  adrs: []
phase: 3
runner: cargo-test
runner-args: tc_197_driver_max_iter_and_stuck
runner-timeout: 60
last-run: 2026-05-28T08:49:19.011623470+00:00
last-run-duration: 0.4s
failure-message: "warning: function `handler_internal` is never used\n  --> crates/decision-cli/src/features/loop_inspect/mod.rs:85:4\n   |\n85 | fn handler_internal(detail: String) -> HandlerError {\n   |    ^^^^^^^^^^^^^^^^\n   |\n   = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default\n\nwarning: missing documentation for a variant\n  --> crates/decision-cli/src/core/dispatch_session.rs:37:5\n   |\n37 |     Completed,\n   |     ^^^^^^^^^\n   |\n   = note: requested on the command line with `-W missing-docs"
---

## Claim

`drive::run` enforces the `max_iter` cap and surfaces a planner's `Stuck` reason verbatim:

- When a stubbed planner returns `Action::Done` only after N+1 iterations, calling `run` with `max_iter = N` returns `Err::MaxIterations` whose history has exactly N entries.
- When a stubbed planner returns `Action::Stuck { reason: "X" }`, calling `run` returns `Err::Stuck { reason: "X", history: [...] }` with the reason string preserved exactly.
- Both error variants include the full action history so post-mortem audit can replay the loop.

## Scenarios

### Test A — max-iter cap

A test-only planner that always returns `Action::DispatchVerifier { ... }` (forcing infinite progress with no convergence) is driven with `max_iter = 3`. Assert:

1. The driver returns `Err::MaxIterations`.
2. The history has exactly 3 entries.
3. The dispatch executor in the test harness was called 3 times.

### Test B — Stuck propagation

A test-only planner returns `Action::Stuck { reason: "synthetic-stuck-reason" }` on the first call. Assert:

1. The driver returns `Err::Stuck { reason: "synthetic-stuck-reason", .. }`.
2. The reason string is identical to the planner's input — no rewrapping.
3. History has exactly one entry (the Stuck action itself).
4. The dispatch executor was never called.