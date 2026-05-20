---
id: TC-029
title: VerificationVerdict conforms to ADR-018 SHACL shape
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
runner-args: --package decision-cli --test verdict_shacl
runner-timeout: 180
last-run: 2026-05-20T11:41:36.841111001+00:00
last-run-duration: 1.9s
---

## Description

[Describe test here.]