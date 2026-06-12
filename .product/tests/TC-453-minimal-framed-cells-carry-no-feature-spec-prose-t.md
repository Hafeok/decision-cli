---
id: TC-453
title: Minimal-framed cells carry no feature-spec prose — the witnessed hallucination source is structurally absent
type: invariant
status: passing
validates:
  features: [FT-177]
  adrs: [ADR-091, ADR-080]
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_177_minimal_framing_excludes_spec_body
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T10:20:23.805264847+00:00
last-run-duration: 366.8s
---

## Purpose

FT-177 / ADR-091 load-bearing invariant: a `Minimal`-framed cell's bundle contains **no feature-spec prose** — the witnessed hallucination source (the FT-148 parser cell invented vocab files for types mentioned only in spec text it never needed). The bundle instead instructs the worker that the upstream artifacts are the complete specification.

## Mechanism

`cargo test -p decision-cli ft_177_minimal_framing_excludes_spec_body` — builds a parser-cell bundle with a marker string in the spec body and asserts the marker is absent while the upstream struct is present.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero — spec prose leaked into an SPMC bundle.