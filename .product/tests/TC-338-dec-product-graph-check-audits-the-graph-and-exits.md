---
id: TC-338
title: dec product graph check audits the graph and exits cleanly when there are zero errors
type: scenario
status: unimplemented
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/dec-product-verb.sh graph check
runner-timeout: 60
observes:
- stdout
- exit-code
---

## Acceptance criteria

Verifies that `dec product graph check` (as wired in [FT-136](FT-136) §Phase 2) invokes `product_core::graph::full_check::run` and reports the audit result.

### Conditions

- Run `dec product graph check` against this repo's `.product/`.
- Exits with code `0` if the graph has zero errors; non-zero exit if errors are present (warnings are allowed).
- stdout contains evidence of the check — at minimum one of: substring `errors`, `warnings`, `clean`, a count, or an error message.

### Surface

`stdout`, `exit-code` — bash script.
