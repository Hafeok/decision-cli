---
id: TC-470
title: Topo levels group cells by dependency depth deterministically for every registered TaskType
type: invariant
status: passing
validates:
  features: [FT-181]
  adrs: [ADR-080, ADR-091]
phase: 1
runner: cargo-test
runner-args: -p dec-harness ft_181_topo_levels_for_add_artifact_type
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T13:19:58.955295907+00:00
last-run-duration: 0.2s
---

## Purpose

FT-181: `topo_levels` groups cells by dependency depth for the real add-artifact-type registry (rust_struct → shacl/iri → parser/emitter + negative tests → round-trip test), every cell exactly once, byte-deterministic across calls.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.