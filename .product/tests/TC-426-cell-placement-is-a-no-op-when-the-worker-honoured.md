---
id: TC-426
title: Cell placement is a no-op when the worker honoured the resolved path
type: invariant
status: passing
validates:
  features: [FT-170]
  adrs: [ADR-008, ADR-080]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_170_placement_noop_when_resolved_path_written
runner-timeout: 300
observes:
- exit-code
- stdout
- disk-state
last-run: 2026-06-11T18:05:26.271295372+00:00
last-run-duration: 1.5s
---

## Purpose

FT-170 case 1 — when the worker honoured the resolved `output_path`, placement is a strict no-op: the file's content is untouched and stray extra files of the same kind are tolerated (they do not trigger ambiguity once the resolved path exists).

## Mechanism

`cargo test -p decision-cli ft_170_placement_noop_when_resolved_path_written`.

## Pass criteria

Observed surfaces: exit-code, stdout, disk-state. Exit-code 0 — resolved-path content byte-identical after placement.

## Fail criteria

Exit-code non-zero — placement touched a correctly-placed artifact.