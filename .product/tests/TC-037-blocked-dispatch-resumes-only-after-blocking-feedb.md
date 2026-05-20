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
last-run: 2026-05-20T08:39:57.378014998+00:00
last-run-duration: 0.0s
failure-message: "bash: line 1: scripts/checks/feedback-resume-on-addressed.sh: No such file or directory\n"
---

## Description

[Describe test here.]