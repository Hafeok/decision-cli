---
id: TC-250
title: dec _activate-binding errors out when activating a binding that would create duplicate-active state
type: scenario
status: passing
validates:
  features:
  - FT-118
  adrs: []
observes:
- stderr
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-250-activate-binding-conflict.sh
runner-timeout: 30
last-run: 2026-05-29T19:57:40.853088466+00:00
last-run-duration: 0.0s
---

## Description

The activate-binding CLI is the manual recovery path. It
must atomically activate the chosen version AND deactivate
all other versions for the role — never leave duplicate-
active state behind. If it can't perform both halves of the
transaction (e.g., one of the prior bindings doesn't exist),
it fails fast rather than half-applying.

## Acceptance Criteria

Bash test:

1. Compose a temp workdir with two bindings for role R:
   - `<binding/R/v1>` active, default_capability=A
   - `<binding/R/v7>` inactive, default_capability=B
2. Run `dec _activate-binding --role R --version 7`. Assert
   exit 0.
3. Assert post-state:
   - v7 active, v1 inactive.
   - Exactly one active binding for R.
4. Run `dec _activate-binding --role R --version 99` (no
   such binding). Assert exit non-zero, stderr contains
   "no such binding".
5. Assert NO state change happened — v7 stayed active, v1
   stayed inactive.
6. Inject a fault: simulate the deactivation transaction
   failing (e.g. stub StreamWriter that errors on the
   deactivate write). Run `_activate-binding --role R
   --version 1`. Assert: exit non-zero, v1 stays inactive,
   v7 stays active.