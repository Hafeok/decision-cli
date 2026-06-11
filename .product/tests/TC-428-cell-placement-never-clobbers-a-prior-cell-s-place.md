---
id: TC-428
title: Cell placement never clobbers a prior cell's placed output
type: invariant
status: passing
validates:
  features: [FT-170]
  adrs: [ADR-008, ADR-080]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_170_placement_refuses_to_overwrite_prior_output
runner-timeout: 300
observes:
- exit-code
- stdout
- disk-state
last-run: 2026-06-11T18:05:26.271295372+00:00
last-run-duration: 0.7s
---

## Purpose

FT-170 invariant — relocation refuses to overwrite a file that existed before the cell ran. A prior cell's placed output is never clobbered by a later cell's drift; the collision is a loud cell failure.

## Mechanism

`cargo test -p decision-cli ft_170_placement_refuses_to_overwrite_prior_output`.

## Pass criteria

Observed surfaces: exit-code, stdout, disk-state. Exit-code 0 — the collision is refused and the pre-existing file's content is untouched.

## Fail criteria

Exit-code non-zero — the prior output was overwritten or the collision passed silently.