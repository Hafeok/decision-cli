---
id: TC-475
title: ApplicationContract negative SHACL — missing required convention and empty body_path rejected
type: invariant
status: passing
validates:
  features: [FT-148]
  adrs: [ADR-082]
phase: 1
runner: cargo-test
runner-args: -p dec-ontology -- missing_required_convention_fails_shacl empty_body_path_fails_shacl
runner-timeout: 300
observes:
- exit-code
- stdout
last-run: 2026-06-12T13:34:22.523452111+00:00
last-run-duration: 0.8s
---

## Purpose

FT-148 spec tests 2-3: a missing required Convention link (languageRuntime) and a Convention with an empty body_path are both rejected by the pure validator with field-naming reports.

## Pass criteria

Observed surfaces: exit-code and stdout. Exit-code 0.

## Fail criteria

Exit-code non-zero.