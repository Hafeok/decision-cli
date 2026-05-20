---
id: TC-038
title: closed feedback references its addressing artifact via PROV-O
type: invariant
status: passing
validates:
  features:
  - FT-027
  adrs:
  - ADR-024
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test feedback_closed_provo
runner-timeout: 120
last-run: 2026-05-20T12:07:06.198265329+00:00
last-run-duration: 0.3s
---

## Description

[Describe test here.]