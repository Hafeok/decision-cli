---
id: TC-034
title: Feedback class is in the controlled vocabulary
type: invariant
status: passing
validates:
  features:
  - FT-028
  adrs:
  - ADR-023
phase: 2
runner: cargo-test
runner-args: --package decision-cli --test feedback_class_vocab
runner-timeout: 120
last-run: 2026-05-20T12:12:50.842701986+00:00
last-run-duration: 0.2s
---

## Description

[Describe test here.]