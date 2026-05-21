---
id: TC-093
title: feedback class vocabulary rejects unknown class literals at write time
type: exit-criteria
status: unimplemented
validates:
  features:
  - FT-028
  adrs:
  - ADR-023
phase: 2
runner: cargo-test
runner-args: tc_093_feedback_class_vocabulary_rejection
runner-timeout: 120
---

## Purpose

Exit criterion for [FT-028](FT-028): the writer chokepoint refuses to persist a `dec:Feedback` artifact whose `dec:class` is not in the controlled vocabulary ([ADR-023](ADR-023)).

## Given

A clean store. A writer call attempting to commit a feedback artifact with `class = "free-form-classification"` (not in the vocabulary).

## When

```rust
let result = writer.commit_feedback(Feedback {
    class: "free-form-classification".into(),
    // ...other fields valid
});
```

## Then

- `result` is `Err(Error::SchemaViolation { detail })`.
- The error message names the offending class value and the allowed set.
- No partial artifact is observable in the store afterwards.

## Notes

Pairs with invariant TC-034 (feedback class in vocabulary). TC-034 asserts "every persisted feedback has a valid class"; TC-093 asserts "the writer is the chokepoint that enforces this".
