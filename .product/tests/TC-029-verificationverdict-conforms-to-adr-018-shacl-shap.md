---
id: TC-029
title: VerificationVerdict conforms to ADR-018 SHACL shape
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
runner-args: --package decision-cli --test verdict_shacl
runner-timeout: 180
---

## Description

[Describe test here.]
