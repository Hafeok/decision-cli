---
id: FT-052
title: 'decision-cli: dec preflight command reading the internal product graph'
phase: 2
status: planned
depends-on: []
adrs:
- ADR-031
- ADR-011
tests: []
domains: []
domains-acknowledged: {}
---

## Description

Add `dec preflight FT-XXX` — a feature-coverage and gap report that reads the
internal product-cli graph (the one rooted at `.product/`) as the **source of
truth**, not the markdown files on disk. The output matches `product preflight`
exactly on `cross_cutting_gaps`, `domain_gaps`, and `dep_availability`, so a
contributor and a CI gate see the same answer to "is FT-XXX ready to dispatch?".

This feature is the consistency claim that lets `dec` trust the internal graph
for chain-integrity checks (ADR-031, FT-047) without re-reading markdown on
every dispatch. It is the prerequisite for treating the graph as the dispatch
gate rather than the markdown directory.

## Why this exists separately

TC-087 originally validated FT-015. FT-015's actual deliverables — adopting
ADR-014's rules-live-in-the-internal-graph convention, shipping ADR-013 as the
first inhabitant, and documenting the lifecycle in CLAUDE.md — were satisfied
without `dec preflight` existing. The TC was speculatively scoped to FT-015
when in fact it describes a `dec` command surface that has not been built.

This feature_spec separates the convention (FT-015, complete) from the command
surface (FT-052, planned) and reparents TC-087 onto the feature that actually
delivers what the TC tests.

## Functional Specification

### Inputs

- A working tree initialised via `dec init` with `.product/` populated.
- The product-cli graph store under `.product/.store/` (if product-cli ships
  one) or the canonical projection of `.product/` markdown frontmatter into
  RDF.
- A feature ID (e.g. `FT-007`).

### Outputs

- A structured report on stdout containing at minimum:
  - `cross_cutting_gaps`: cross-cutting ADRs the feature has not acknowledged.
  - `domain_gaps`: domain ADRs missing for the feature's domain set.
  - `dep_availability`: depends-on features and their current status.
- Exit 0 when there are no blocking gaps; exit 1 when any gap blocks dispatch.
- Output equivalent in structure to `product preflight FT-XXX` so a mismatch
  is mechanically detectable.

### Source-of-truth contract

`dec preflight` **must not re-parse the markdown files** at call time. It reads
the graph projection. Verified by mutating a frontmatter field while the
projection is unchanged: `dec preflight` returns the projected view, not the
mutated markdown. This is the load-bearing claim TC-087 validates.

## Scope

In scope:

- `dec preflight FT-XXX` subcommand wired into `main.rs` per the ADR-011 CLI
  shape.
- Graph reader returning the structured report.
- Integration test (cargo) under `crates/decision-cli/src/features/preflight/`
  matching the canonical `product preflight` shape.

Out of scope:

- The chain-integrity dispatch gate itself (FT-047).
- Auto-dispatching anything based on preflight results.
- Changes to product-cli's preflight implementation.

## Acceptance

TC-087 passes: `dec preflight FT-007` and `product preflight FT-007` produce
structurally equivalent reports, and the trace assertion (or markdown-mutation
control) demonstrates `dec` reads the graph rather than the markdown.

## Notes

- Citation chain: TC-087's notes reference ADR-031 (chain-integrity invariant),
  which is implemented by FT-047. FT-052 is the prerequisite that makes FT-047
  buildable on a trusted graph projection.
- Phase 2 placement: aligned with the slice 2.5 verification-graph work that
  introduces ADR-031.