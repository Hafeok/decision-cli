---
id: TC-041
title: worker SDK emit_feedback produces a valid Feedback artifact in session telemetry
type: exit-criteria
status: failing
validates:
  features:
  - FT-030
  - FT-031
  adrs:
  - ADR-022
phase: 2
runner: pytest
runner-args: workers/_shared/tests/test_emit_feedback.py
runner-timeout: 120
---

## Description

[Describe test here.]
