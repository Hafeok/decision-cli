---
id: TC-439
title: v1 audit without parseable verdict is unrunnable, legacy-text falls back with noncompliant mark
type: scenario
status: unimplemented
validates:
  features: []
  adrs: []
phase: 1
runner: cargo-test
runner-args: -p decision-cli ft_173_verdict_grandfathering
runner-timeout: 300
observes:
- graph
- exit-code
---

## Description

Two halves of the grandfathering contract (ADR-087 §2/§6):

1. An audit declared `verdict: v1` exits 1 but writes no parseable verdict document — the cluster run must treat the audit as `unrunnable` (**exit-code** surface: the run fails with the unrunnable diagnostic, never a silent pass and never a text-parse fallback).
2. The same failing audit declared `verdict: legacy-text` falls back to today's diagnostic text extraction, and the persisted SessionRecord carries the `verdict-noncompliant` mark (**graph** surface).
