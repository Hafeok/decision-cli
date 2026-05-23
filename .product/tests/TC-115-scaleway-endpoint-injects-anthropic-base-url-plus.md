---
id: TC-115
title: Scaleway endpoint injects ANTHROPIC_BASE_URL plus AUTH_TOKEN plus pinned model env vars on the claude-p subprocess
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: pytest
runner-args: workers/code-writer/tests/test_claude_env_routing.py::test_scaleway_env
runner-timeout: 60
last-run: 2026-05-23T18:00:14.922758228+00:00
last-run-duration: 0.4s
---

## Description

[Describe the test criterion here.]