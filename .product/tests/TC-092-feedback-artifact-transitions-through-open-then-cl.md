---
id: TC-092
title: feedback artifact transitions through open then closed via the writer
type: exit-criteria
status: passing
validates:
  features:
  - FT-027
  adrs:
  - ADR-024
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test feedback_lifecycle
runner-timeout: 120
last-run: 2026-05-23T16:10:19.845721788+00:00
last-run-duration: 0.2s
---

## Purpose

Exit criterion for [FT-027](FT-027): a `dec:Feedback` artifact transitions from `open` to `closed` through the writer-enforced state machine, with `prov:wasInvalidatedBy` set on close.

## Given

A clean store. An open `dec:Feedback` artifact created via the writer with `class: gap`, `subject: <some-artifact>`, no `addressedBy` set, `state: open`.

## When

```rust
let closed = writer.close_feedback(feedback_id, addressing_artifact_id)?;
```

## Then

- The artifact's `dec:state` is now `closed`.
- `dec:addressedBy` is set to `addressing_artifact_id`.
- `prov:wasInvalidatedBy` references the closure activity ([ADR-004](ADR-004) PROV-O).
- Attempting the same close a second time returns `Error::IllegalTransition` (idempotent close is not supported by design — close is a one-way state change).

## Notes

Pairs with invariant TC-035 (transition validation) and TC-038 (closed-feedback references its addressing artifact). The exit-criterion roll-up validates the writer surface itself.