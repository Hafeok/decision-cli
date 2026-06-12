---
id: TC-327
title: 'FT-135: dec drive def-ready --all streams per-feature outcome line before sweep completes'
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_135_outcome_line_on_done
runner-timeout: 300
observes:
- stderr
last-run: 2026-06-12T12:53:43.885714192+00:00
last-run-duration: 0.7s
---

## Description

[Describe the test criterion here.]