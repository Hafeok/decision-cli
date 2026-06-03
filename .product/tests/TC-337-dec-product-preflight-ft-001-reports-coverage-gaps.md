---
id: TC-337
title: dec product preflight FT-001 reports coverage gaps as structured output
type: scenario
status: unimplemented
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/dec-product-verb.sh preflight FT-001
runner-timeout: 60
observes:
- stdout
- exit-code
---

## Acceptance criteria

Verifies that `dec product preflight FT-001` (as wired in [FT-136](FT-136) §Phase 2) invokes `product_core::gap::check::check_feature_dep_gaps` (or the equivalent) and surfaces the coverage report.

### Conditions

- Run `dec product preflight FT-001` against this repo's `.product/`.
- Exits with code `0` (preflight findings are not errors — they're warnings/info).
- stdout contains the feature ID `FT-001`.
- stdout contains evidence of preflight output — at minimum one of: substring `gap`, `coverage`, `clean`, or a domain name.

### Surface

`stdout`, `exit-code` — bash script.
