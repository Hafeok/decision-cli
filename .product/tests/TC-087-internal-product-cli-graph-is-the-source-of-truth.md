---
id: TC-087
title: internal product-cli graph is the source of truth for dec preflight
type: exit-criteria
status: unrunnable
validates:
  features:
  - FT-015
  adrs: []
phase: 1
runner: cargo-test
runner-args: tc_087_internal_product_graph_source_of_truth
runner-timeout: 120
---

## Purpose

Exit criterion for [FT-015](FT-015): `dec preflight FT-XXX` reads the internal product-cli graph (not the markdown files) as the source of truth, matching `product preflight` exactly on coverage and gaps.

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
