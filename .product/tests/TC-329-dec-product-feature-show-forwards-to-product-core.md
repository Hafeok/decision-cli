---
id: TC-329
title: dec product feature show forwards to product_core and returns the expected artifact
type: scenario
status: passing
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: cargo-test
runner-args: --package decision-cli --test ft_136_product_forwards
runner-timeout: 120
observes:
- stdout
- exit-code
last-run: 2026-06-03T12:09:07.168277302+00:00
last-run-duration: 1.1s
---

## Acceptance criteria

Verifies that [FT-136](FT-136)'s `dec product feature show <ID>` adapter forwards in-process to `product_core::feature::show` and surfaces the artifact's identifying fields.

The test is **behavioural parity, not byte parity** with the upstream `product feature show` binary — rendering format may diverge; the artifact's identity must surface.

### Conditions

- Given a fixture `.product/` checkout containing a feature with ID `FT-001` and title `"oxi-events: GraphWriter mutation chokepoint"` (the actual phase-1 seed).
- Running `dec product feature show FT-001` against that fixture:
  - exits with code `0`.
  - prints output to stdout containing the literal string `FT-001`.
  - prints output to stdout containing the feature's title (substring match — exact whitespace/punctuation may differ from upstream).
- Running `dec product feature show FT-NONEXISTENT`:
  - exits with a non-zero code.
  - prints a diagnostic to stderr (presence asserted; exact text not constrained).

### Surface

`stdout`, `exit-code` — integration test boots the `dec` binary via `assert_cmd` or equivalent.