---
id: TC-326
title: 'FT-135: --quiet suppresses progress on stderr while leaving stdout history dump intact'
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_135_quiet_sink_constructs_and_dispatches
runner-timeout: 300
observes:
- stderr
last-run: 2026-06-12T12:53:43.885714192+00:00
last-run-duration: 0.8s
---

## Description

[Describe the test criterion here.]