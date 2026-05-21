---
id: TC-092
title: feedback artifact transitions through open then closed via the writer
type: exit-criteria
status: failing
validates:
  features:
  - FT-027
  adrs:
  - ADR-024
phase: 2
runner: cargo-test
runner-args: tc_092_feedback_transition_open_to_closed
runner-timeout: 120
last-run: 2026-05-21T13:31:41.447481757+00:00
last-run-duration: 2.7s
failure-message: "   Compiling decision-cli v0.1.0 (/home/hafeok/projects/decision-cli/crates/decision-cli)\nerror[E0432]: unresolved import `decision_cli::core::ontology::verification_env`\n  --> crates/decision-cli/tests/tc_055_dec_init_seeds_ephemeral_cli_env_idempotently.rs:16:35\n   |\n16 | use decision_cli::core::ontology::verification_env::{\n   |                                   ^^^^^^^^^^^^^^^^ could not find `verification_env` in `ontology`\n\nerror[E0432]: unresolved import `decision_cli::core::ontology::ver"
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