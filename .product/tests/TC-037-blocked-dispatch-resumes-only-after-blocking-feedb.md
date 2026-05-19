---
id: TC-037
title: blocked dispatch resumes only after blocking feedback is addressed
type: exit-criteria
status: failing
validates:
  features:
  - FT-032
  adrs:
  - ADR-025
phase: 2
runner: bash
runner-args: scripts/checks/feedback-resume-on-addressed.sh
runner-timeout: 180
---

## Description

[Describe test here.]
