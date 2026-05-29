---
id: TC-251
title: dec _deactivate-binding flips active to false idempotently and preserves binding history
type: scenario
status: passing
validates:
  features:
  - FT-118
  adrs: []
observes:
- graph
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-251-deactivate-binding-idempotent.sh
runner-timeout: 30
last-run: 2026-05-29T19:57:40.853088466+00:00
last-run-duration: 0.0s
---

## Description

The deactivate CLI is the looser counterpart to
`_activate-binding`. It allows operators to remove the
"active" flag from a binding without immediately switching
in a replacement. Important properties: idempotent on
already-inactive bindings (no error, no change), preserves
binding history (only the `active` flag flips), works even
when no other binding for the role exists (zero-active is
acceptable per FT-118 §Invariants).

## Acceptance Criteria

Bash test:

1. Compose a temp workdir with one binding:
   `<binding/R/v1>` active, default_capability=A,
   plus a `dec:roleId` quad and a creation-timestamp quad.
2. Run `dec _deactivate-binding --role R --version 1`.
   Assert exit 0.
3. Assert post-state:
   - `dec:active=false` on `<binding/R/v1>`.
   - Every other quad on `<binding/R/v1>` preserved
     byte-identical (default_capability, roleId, timestamps).
   - The "binding history" is intact — the deactivate flips
     a flag, never removes the binding.
4. Re-run the same `_deactivate-binding` command. Assert
   exit 0, stdout contains "no-op" (idempotent). Quad set
   on `<binding/R/v1>` unchanged.
5. Run `_deactivate-binding --role R --version 99` (no
   such binding). Assert exit non-zero, stderr "no such
   binding".