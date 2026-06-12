---
id: TC-458
title: Declared context files exist and the real Provenance surface distills without bodies
type: invariant
status: passing
validates:
  features: [FT-178]
  adrs: [ADR-091]
phase: 1
runner: cargo-test
runner-args: -p decision-cli -- ft_178_registry_context_files_exist_on_disk ft_178_real_provenance_surface_distills
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T10:51:13.853263711+00:00
last-run-duration: 7.5s
---

## Purpose

FT-178: the registry-declared context files exist in the live tree, and distilling the real `provenance.rs` yields `pub struct Provenance` + `pub fn to_quads` signatures with implementation bodies dropped. Runner: two targeted tests.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.