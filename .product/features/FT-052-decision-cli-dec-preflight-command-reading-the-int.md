---
id: FT-052
title: 'decision-cli: dec preflight command reading the internal product graph'
phase: 2
status: planned
depends-on: []
adrs:
- ADR-031
- ADR-011
tests:
- TC-087
domains: []
domains-acknowledged:
  ADR-001: ADR-001 governs the oxi-events crate boundary; FT-052 reads product-cli's graph projection and does not consume or cross the oxi-events surface.
  ADR-012: ADR-012 (per-stream working directory discovery) governs CLI entry; FT-052 runs after the working directory is resolved at command entry and does not re-discover it.
  ADR-017: ADR-017 (action-interpretation pairing) is implemented by FT-021; FT-052 is a read-only command and produces no action/interpretation pair.
  ADR-025: ADR-025 (blocking vs non-blocking feedback semantics) is implemented by FT-032; FT-052 has no feedback to gate.
  ADR-021: ADR-021 (action-interpretation agreement metric) is a Slice 2 fitness function implemented by FT-024; FT-052 is a read-only command and produces no action/interpretation pair.
  ADR-005: ADR-005 (value-stream scope) governs command-time scope; `dec preflight` runs inside an already-scoped command and introduces no new scope check.
  ADR-004: ADR-004 (PROV-O) governs session/event lineage; `dec preflight` is a pure read and opens no session.
  ADR-018: ADR-018 (VerificationVerdict schema) is implemented by FT-020; FT-052 neither emits nor consumes verdicts.
  ADR-013: ADR-013 (code structure standards) applies workspace-wide; FT-052's code conforms to cargo/clippy/rustfmt and the module-size convention. ADR-013 itself is owned by FT-014.
  ADR-002: 'ADR-002 (graph-as-state) is the load-bearing premise of FT-052: the preflight reader treats the internal product-cli graph projection as authoritative state and performs no writes.'
  ADR-014: ADR-014 (fitness functions tracked as artifacts) is owned by FT-014/FT-015; FT-052 reads the cross-cutting projection produced by that convention but does not author or modify a fitness-function artifact.
  ADR-023: ADR-023 (feedback class controlled vocabulary) is implemented by FT-028; FT-052 produces no feedback artifacts.
  ADR-024: ADR-024 (feedback lifecycle state machine) is implemented by FT-027; FT-052 produces no feedback artifacts.
  ADR-016: ADR-016 (vertical-slice + compile-time SDP) is migrated by FT-018; FT-052's handler lives in its own `features/preflight/` slice and accesses only `core/` substrate, with no cross-feature imports.
  ADR-027: ADR-027 (authority declarations in role catalog) is implemented by FT-030; FT-052 does not introduce or modify a role catalog entry.
  ADR-022: ADR-022 (feedback as a first-class flow class) is implemented by FT-026; FT-052 is a read-only preflight report and emits no feedback.
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