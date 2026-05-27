---
id: TC-089
title: VerificationVerdict round-trips through writer and reader
type: exit-criteria
status: passing
validates:
  features:
  - FT-020
  adrs:
  - ADR-018
phase: 2
runner: cargo-test
runner-args: -p decision-cli --test verdict_shacl
runner-timeout: 120
last-run: 2026-05-27T13:44:12.255848478+00:00
last-run-duration: 0.2s
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