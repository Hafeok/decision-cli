---
id: TC-427
title: Cell placement fails loudly on missing or ambiguous output — never guesses
type: invariant
status: passing
validates:
  features: [FT-170]
  adrs: [ADR-008, ADR-080]
phase: 1
runner: cargo-test
runner-args: -p decision-cli -- ft_170_placement_fails_when_nothing_of_right_kind ft_170_placement_fails_on_ambiguous_candidates
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-11T18:55:52.916349280+00:00
last-run-duration: 2.8s
---

## Purpose

FT-170 cases 3 and 4 — placement never guesses. A cell that produced nothing of the right kind fails with a diagnostic naming the expected path (previously this surfaced only later, at read-back or audit time); a cell that produced several candidates and none at the resolved path fails with a diagnostic listing every candidate.

## Mechanism

`cargo test -p decision-cli -- ft_170_placement_fails_when_nothing_of_right_kind ft_170_placement_fails_on_ambiguous_candidates`.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — both refusals fire with their respective diagnostics.

## Fail criteria

Exit-code non-zero — a missing or ambiguous output was silently accepted or resolved.