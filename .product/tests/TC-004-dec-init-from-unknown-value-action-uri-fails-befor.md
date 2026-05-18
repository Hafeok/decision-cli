---
id: TC-004
title: dec_init_from_unknown_value_action_uri_fails_before_write
type: exit-criteria
status: passing
validates:
  features: []
  adrs: []
phase: 1
runner: bash
runner-args: tests/scripts/tc-004-unknown-value-action-uri.sh
runner-timeout: 30
last-run: 2026-05-18T19:02:15.368687079+00:00
last-run-duration: 0.2s
---

## Purpose

Validates the resolve step of the **ADR-006** pipeline against the bundled definition library (FT-007): a definition that references a `dec:terminalValueAction` URI not in the bundled set must fail **before** writing state, with the unresolvable URI named.

Source: `decision-cli-slice-1-bounds.md` §11.2 exit-criteria #4.

## Given

- A fresh working directory with no `.dec/`.
- A `unknown-action.ttl` that is otherwise SHACL-valid but declares `dec:terminalValueAction <https://example.org/value-actions/not-bundled>`.

## When

```bash
dec init --from ./unknown-action.ttl
```

## Then

1. The command exits non-zero.
2. stderr names the unresolvable URI (`https://example.org/value-actions/not-bundled`) and explains that slice 1 only resolves bundled URIs (per ADR-006 / §6.2).
3. **No `.dec/` directory is created.**

## Notes

- Slice 1 supports only bundled templates and local file paths (ADR-006); network resolution is deferred per §6.2.
- TC-007 confirms the related goal-validation failure mode.