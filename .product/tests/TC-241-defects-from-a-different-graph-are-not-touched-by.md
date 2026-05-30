---
id: TC-241
title: Defects from a different graph are not touched by auto-close
type: invariant
status: passing
validates:
  features:
  - FT-116
  adrs: []
phase: 4
runner: cargo-test
runner-args: features::ft_116_retract_stale_defects::tests::tc_241_defects_from_different_graph_not_touched
runner-timeout: 30
observes:
- graph
last-run: 2026-05-30T11:19:08.836590639+00:00
last-run-duration: 0.4s
---

## Description

Two graphs may both cover TC T but exercise it in different
ways (different commands, different benches). An approved VGR
on graph G says nothing about graph G's evidence for T —
distinct evidence streams. Auto-close must respect this
boundary; otherwise a passing run on G could mask a real
failure on G'.

## Acceptance Criteria

Cargo test:

1. Seed two graphs both covering TC T:
   - Graph G (`VG-100`), VGR-1 for G emitted defect fb-G
     for T.
   - Graph G' (`VG-200`), VGR-1' for G' emitted defect fb-G'
     for T.
2. Write VGR-2 for G with `outcome="pass"` for T (the new
   approved evidence is from G only).
3. Invoke `retract_stale_defects(store, VGR-2)`.
4. Assert:
   - fb-G is `closed` (G's evidence flipped).
   - fb-G' is still `produced` (G' has not been re-verified;
     its evidence stream is unchanged).
   - The SPARQL query in `query.rs` uses graph_iri as a
     positive filter, not just tc_iri.