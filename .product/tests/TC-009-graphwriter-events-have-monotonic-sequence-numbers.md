---
id: TC-009
title: graphwriter_events_have_monotonic_sequence_numbers
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p oxi-events --test tc_009_monotonic_events
runner-timeout: 60
last-run: 2026-05-18T18:23:38.378348108+00:00
last-run-duration: 0.9s
---

## Purpose

Validates **ADR-002** (graph-as-state, events live in graph) and the FT-001 GraphWriter contract: events emitted on commit must be queryable via SPARQL with **monotonic, contiguous** sequence numbers.

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #9.

## Given

- An open `OrchestrationStore` (FT-009) with the `GraphWriter` (FT-001) wired to the subscription evaluator (FT-002) and the event outbox (FT-003).
- At least one registered subscription whose triggers will be hit by the mutations below.

## When

Perform a controlled sequence of N mutations through `GraphWriter` (N ≥ 5).

## Then

1. SPARQL against the events graph:
   ```sparql
   SELECT ?event ?seq WHERE { ?event a oxi:Event ; oxi:seq ?seq } ORDER BY ?seq
   ```
   returns at least one event per mutation (depending on subscription matches) and possibly more.
2. The returned `?seq` values are **strictly increasing** with no gaps within the run.
3. Every returned event carries a `prov:wasGeneratedBy` triple pointing to a Mutation node that exists in the store (ADR-004 invariant).
4. Re-running the SPARQL after a process restart (without further mutations) returns the **same** events in the same order — confirming events are persisted, not transient.

## Notes

- The contiguity property is asserted in FT-001's Invariants section.
- TC-010 complements this with crash-recovery semantics for unpublished events.