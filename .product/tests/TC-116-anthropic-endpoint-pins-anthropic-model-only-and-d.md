---
id: TC-116
title: Anthropic endpoint pins ANTHROPIC_MODEL only and does not set ANTHROPIC_BASE_URL on the claude-p subprocess
type: scenario
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: pytest
runner-args: workers/code-writer/tests/test_claude_env_routing.py::test_anthropic_env
runner-timeout: 60
last-run: 2026-05-23T17:33:21.436872146+00:00
last-run-duration: 0.4s
---

## Description

[Describe the test criterion here.]