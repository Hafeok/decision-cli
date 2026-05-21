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
last-run: 2026-05-21T13:31:41.447481757+00:00
last-run-duration: 0.1s
---

## Description

[Describe test here.]