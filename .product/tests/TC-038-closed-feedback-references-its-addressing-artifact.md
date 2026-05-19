---
id: TC-038
title: closed feedback references its addressing artifact via PROV-O
type: invariant
status: failing
validates:
  features:
  - FT-027
  adrs:
  - ADR-024
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test feedback_closed_provo
runner-timeout: 120
---

## Description

[Describe test here.]
