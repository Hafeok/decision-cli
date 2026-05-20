---
id: TC-030
title: rejected verdict cites at least one TC or ADR
type: invariant
status: passing
validates:
  features:
  - FT-020
  - FT-023
  adrs:
  - ADR-018
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test verdict_rejected_cites
runner-timeout: 120
last-run: 2026-05-20T09:56:30.698521759+00:00
last-run-duration: 0.1s
---

## Description

[Describe test here.]