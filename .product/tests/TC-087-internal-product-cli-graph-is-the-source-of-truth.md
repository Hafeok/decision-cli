---
id: TC-087
title: internal product-cli graph is the source of truth for dec preflight
type: exit-criteria
status: passing
validates:
  features:
  - FT-052
  adrs: []
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test tc_087_internal_product_graph_source_of_truth
runner-timeout: 120
last-run: 2026-05-24T19:14:13.655371796+00:00
last-run-duration: 0.2s
---

## Purpose

Exit criterion for [FT-052](FT-052): `dec preflight FT-XXX` reads the internal product-cli graph (not the markdown files) as the source of truth, matching `product preflight` exactly on coverage and gaps.

This TC was originally scoped to FT-015 (the rules-live-in-`.product/` convention). FT-015's deliverables were satisfied without `dec preflight` existing — the command surface is separate work. FT-052 is the feature that actually delivers it; TC-087 was reparented there in phase 2.

## Given

A working directory initialized with `dec init`, with at least one feature spec authored. The product-cli graph store under `.product/.store/` exists and matches the markdown frontmatter.

## When

```bash
dec preflight FT-007
product preflight FT-007
```

## Then

- The two outputs are equivalent in structure: same `cross_cutting_gaps` set, same `domain_gaps`, same `dep_availability`.
- `dec preflight` does **not** re-parse the markdown — verified by a trace assertion or by mutating a frontmatter field while the graph projection is unchanged: `dec preflight` returns the graph-projected view, not the mutated markdown.

## Notes

This is the consistency claim that lets `dec` trust the internal graph for chain-integrity checks ([ADR-031](ADR-031)) without re-reading markdown on every dispatch.