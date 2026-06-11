---
id: TC-418
title: ActiveScope integration — relocated scope tests pass through the real dec init path via the dec-graph facade
type: exit-criteria
status: passing
validates:
  features:
  - FT-168
  adrs:
  - ADR-086
phase: 1
runner: cargo-test
runner-args: --test scope_integration
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T13:46:50.631871209+00:00
last-run-duration: 0.5s
---

## Purpose

Exit criterion for [FT-168](FT-168) ([ADR-086](ADR-086)): `ActiveScope` moved into dec-graph, but its behaviour is only meaningful through the real `dec init` path — template bootstrap, store discovery, goal authorization. These tests were relocated from `core/scope/tests.rs` to a decision-cli integration test (`tests/scope_integration.rs`) because they consume the binary crate's init machinery, which dec-graph must not depend on.

## Mechanism

`cargo test --test scope_integration` — initializes a stream from the bundled engineering-development template in a tempdir, then exercises `decision_cli::core::scope::ActiveScope` (load, goal authorization pass/fail, uninitialized-workdir error) through the dec-graph facade.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — scope behaviour is identical through the facade.

## Fail criteria

Exit-code 1 — a scope behaviour changed during the relocation; stdout carries the cargo test report.