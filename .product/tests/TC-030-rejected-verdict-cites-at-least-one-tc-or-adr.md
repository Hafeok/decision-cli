---
id: TC-030
title: rejected verdict cites at least one TC or ADR
type: invariant
status: failing
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
---

## Description

[Describe test here.]
