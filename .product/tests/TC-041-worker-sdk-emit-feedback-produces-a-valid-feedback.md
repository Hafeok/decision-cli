---
id: TC-041
title: worker SDK emit_feedback produces a valid Feedback artifact in session telemetry
type: exit-criteria
status: passing
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
last-run: 2026-05-23T18:00:16.213042199+00:00
last-run-duration: 0.4s
---

## Description

[Describe test here.]