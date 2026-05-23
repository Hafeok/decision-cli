---
id: TC-037
title: blocked dispatch resumes only after blocking feedback is addressed
type: exit-criteria
status: passing
validates:
  features:
  - FT-032
  adrs:
  - ADR-025
phase: 2
runner: bash
runner-args: scripts/checks/feedback-resume-on-addressed.sh
runner-timeout: 180
last-run: 2026-05-23T16:10:19.845721788+00:00
last-run-duration: 0.1s
---

## Description

[Describe test here.]