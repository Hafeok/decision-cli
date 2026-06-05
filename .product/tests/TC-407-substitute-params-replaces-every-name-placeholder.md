---
id: TC-407
title: substitute_params replaces every name placeholder against the resolved map
type: exit-criteria
status: passing
validates:
  features:
  - FT-166
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli --lib features::drive::cluster_dispatch::tests::ft_166_substitute_params_replaces_placeholders
runner-timeout: 120
observes:
- graph
last-run: 2026-06-05T12:27:44.473444561+00:00
last-run-duration: 0.2s
---

## Description

Exit-criteria test for [FT-166](FT-166) §Behaviour parameter substitution. Pins the literal-substitution contract: `{name}` placeholders are replaced against the resolved map; unmatched placeholders stay literal.

## Assertions

1. A template with two placeholders (`{crate_path}` + `{artifact_name}`) substitutes both to produce the expected path.
2. An unmatched placeholder (`{unknown}`) stays literal in the output — no panic, no silent drop.

## Runner

`cargo-test` of `features::drive::cluster_dispatch::tests::ft_166_substitute_params_replaces_placeholders`.