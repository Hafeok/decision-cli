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
last-run: 2026-05-20T08:39:57.378014998+00:00
last-run-duration: 0.2s
failure-message: "ERROR: file or directory not found: workers/_shared/tests/test_emit_feedback.py\n\n"
---

## Description

[Describe test here.]