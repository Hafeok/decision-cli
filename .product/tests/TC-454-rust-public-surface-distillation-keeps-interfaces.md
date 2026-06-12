---
id: TC-454
title: Rust public-surface distillation keeps interfaces and drops bodies deterministically
type: invariant
status: passing
validates:
  features: [FT-177]
  adrs: [ADR-091, ADR-080]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_177_distiller_keeps_public_surface_only
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T10:20:23.805264847+00:00
last-run-duration: 3.4s
---

## Purpose

FT-177 / ADR-091: upstream `.rs` context arrives as interface, not implementation. The deterministic distiller keeps `pub struct`/`pub enum` blocks (fields are interface), `pub const`/`pub use`/`pub type` lines, and `pub fn` signatures — and drops fn bodies, private items, and imports.

## Mechanism

`cargo test -p decision-cli ft_177_distiller_keeps_public_surface_only`.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0 — surface kept, bodies and private items absent.

## Fail criteria

Exit-code non-zero.