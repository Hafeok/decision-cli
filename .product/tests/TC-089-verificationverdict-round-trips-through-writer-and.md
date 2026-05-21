---
id: TC-089
title: VerificationVerdict round-trips through writer and reader
type: exit-criteria
status: failing
validates:
  features:
  - FT-020
  adrs:
  - ADR-018
phase: 2
runner: cargo-test
runner-args: tc_089_verdict_round_trip
runner-timeout: 120
last-run: 2026-05-21T15:21:18.505743289+00:00
last-run-duration: 0.2s
failure-message: "No #[test] fn matching 'tc_089_verdict_round_trip' found in tests/*.rs — did you forget to add the integration test?"
---

## Purpose

Exit criterion for [FT-020](FT-020): a `VerificationVerdict` artifact authored in memory (via the writer) serialises to canonical Turtle and round-trips back through the reader to a byte-equal in-memory value. SHACL validation passes ([ADR-018](ADR-018) shape).

## Given

A `VerificationVerdict` value with `kind = approved`, `actionSessionId = some-uuid`, `interpretationSessionId = some-other-uuid`, `cites = [TC-029]`, and the timestamp captured.

## When

```rust
let written = writer.commit(verdict.clone())?;       // through StreamWriter chokepoint
let read_back = reader.load(written.iri)?;
assert_eq!(read_back, verdict);
```

## Then

- The on-disk Turtle conforms to the `dec:VerificationVerdictShape` SHACL shape (verified by TC-029's invariant runner).
- The round-tripped value is byte-equal to the original.
- The verdict's IRI is deterministically derived from `(actionSessionId, interpretationSessionId)`.

## Notes

This is the closure claim: TC-029 (invariant) says "verdicts that exist conform"; TC-089 (exit) says "the writer produces verdicts that exist and round-trip".