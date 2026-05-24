---
id: TC-086
title: code-structure fitness functions all green on a clean tree
type: exit-criteria
status: passing
validates:
  features:
  - FT-014
  adrs:
  - ADR-013
phase: 1
runner: bash
runner-args: scripts/checks/run-all-fitness.sh
runner-timeout: 120
last-run: 2026-05-24T19:14:23.673322616+00:00
last-run-duration: 3.0s
---

## Purpose

Exit criterion for [FT-014](FT-014): the four cross-cutting code-structure fitness functions (file length, function length per language, module structure, single-responsibility comment) all exit 0 on a representative clean tree.

## Given

A clean workspace at HEAD of `main` where every prior fitness-function TC (TC-044, TC-045) is `passing` and no source file exceeds the ADR-013 limits.

## When

```bash
scripts/checks/file-length.sh \
  && scripts/checks/rust-function-length.sh \
  && scripts/checks/python-function-length.sh \
  && scripts/checks/module-structure.sh \
  && scripts/checks/single-responsibility.sh
```

## Then

- The chained exit code is 0.
- No stderr lines mention violations.
- The fitness-function artifacts referenced by [ADR-013](ADR-013) are present in the repo and runnable from `make ff` (or the documented equivalent).

## Notes

FT-014 is `complete`; this TC pairs the existing invariant TCs (TC-044, TC-045) with a single end-to-end roll-up that closes the feature.