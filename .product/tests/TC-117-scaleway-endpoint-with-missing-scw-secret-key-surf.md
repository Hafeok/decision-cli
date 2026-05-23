---
id: TC-117
title: Scaleway endpoint with missing SCW_SECRET_KEY surfaces a structured endpoint_config WorkerError before subprocess spawn
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: pytest
runner-args: workers/code-writer/tests/test_claude_env_routing.py::test_missing_scw_key
runner-timeout: 60
last-run: 2026-05-23T17:33:21.436872146+00:00
last-run-duration: 0.4s
---

## Description

[Describe the test criterion here.]