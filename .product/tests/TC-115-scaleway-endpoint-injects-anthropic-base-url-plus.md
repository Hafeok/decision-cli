---
id: TC-115
title: Scaleway endpoint injects ANTHROPIC_BASE_URL plus AUTH_TOKEN plus pinned model env vars on the claude-p subprocess
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: pytest
runner-args: workers/code-writer/tests/test_claude_env_routing.py::test_scaleway_env
runner-timeout: 60
last-run: 2026-05-24T19:14:22.584356516+00:00
last-run-duration: 0.3s
---

## Description

[Describe the test criterion here.]