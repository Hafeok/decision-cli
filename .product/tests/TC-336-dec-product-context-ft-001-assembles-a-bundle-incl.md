---
id: TC-336
title: dec product context FT-001 assembles a bundle including linked ADRs
type: scenario
status: passing
validates:
  features:
  - FT-136
  adrs: []
phase: 1
runner: bash
runner-args: scripts/checks/dec-product-verb.sh context FT-001
runner-timeout: 60
observes:
- stdout
- exit-code
last-run: 2026-06-03T10:59:22.224178625+00:00
last-run-duration: 0.2s
---

## Acceptance criteria

Verifies that `dec product context FT-001` (as wired in [FT-136](FT-136) §Phase 2) invokes `product_core::context::bundle_feature` (or its sibling) and prints the assembled bundle.

### Conditions

- Run `dec product context FT-001` against this repo's `.product/`.
- Exits with code `0`.
- stdout contains the feature ID `FT-001`.
- stdout is non-empty (the bundle has content beyond the ID).
- stdout contains evidence of an assembled bundle — at minimum one of: a section header (e.g. `#` heading), an ADR reference (substring `ADR-`), or a YAML front-matter block.

### Surface

`stdout`, `exit-code` — bash script.