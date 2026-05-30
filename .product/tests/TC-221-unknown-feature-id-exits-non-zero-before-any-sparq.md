---
id: TC-221
title: Unknown feature ID exits non-zero before any SPARQL query runs
type: scenario
status: passing
validates:
  features:
  - FT-113
  adrs: []
observes:
- stderr
- exit-code
phase: 4
runner: bash
runner-args: tests/scripts/tc-221-drive-show-unknown-feature.sh
runner-timeout: 30
last-run: 2026-05-30T15:30:51.265294838+00:00
last-run-duration: 0.0s
---

## Description

`dec drive show FT-999` against a feature that doesn't exist
in the product graph should fail fast with a clear error
message — not return an empty `Vec<Round>` (that's the
"never driven" empty state, which is different and misleading
for typos).

## Acceptance Criteria

Bash test:

1. Compose a temp `.product/` workspace with FT-X registered
   (so the lookup mechanism is exercised against a real
   product graph).
2. Run `dec drive show FT-999`. Assert exit code is non-zero
   (specifically `1`).
3. Assert stderr contains the substring `"Unknown feature
   FT-999"` and a hint pointing at `product feature list`.
4. Run `dec drive show FT-X` against the same workspace
   (FT-X has never been driven). Assert exit code is `0`
   (not an error — this is the empty-state path from TC-216).
5. Assert FT-X's stdout contains the empty-state paragraph
   from TC-216, NOT the "Unknown feature" error from step 3.

The two cases — unknown ID vs known-but-undriven — must
produce distinct user-facing signals so operators can
distinguish typo from intent.