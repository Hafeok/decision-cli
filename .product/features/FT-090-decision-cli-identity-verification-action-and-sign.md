---
id: FT-090
title: 'decision-cli: identity-verification action and signature-validity verdict'
phase: 3
status: planned
depends-on:
- FT-089
adrs: []
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. The WorkerCurator's bundle needs a structured verdict on whether a candidate image's signature is valid AND whether the signing identity is on the trust list. That verdict is the output of a pure-execution + interpretation action pair, matching the action-interpretation pairing requirement (ADR-017).

## Scope

- Action role: `identity-verifier`. Flavor: `pure_execution + interpretation`.
- Action mechanics (pure_execution side):
  - Runs `cosign verify` against the candidate image's signature using the trust list.
  - Validates that the Rekor entry referenced on the Submission exists and matches.
- Interpretation (separate session, paired with the action per ADR-019):
  - Produces a `SignatureVerdict` artifact with one of these classes:
    - `valid` — signature checks, identity on trust list, Rekor inclusion confirmed.
    - `invalid-signature` — cosign verify failed cryptographically.
    - `untrusted-identity` — signature valid but signer not on trust list.
    - `image-not-found` — registry returned 404 for the candidate ref.
    - `rekor-entry-missing` — referenced Rekor entry doesn't exist or doesn't match.
- The verdict artifact feeds the WorkerCurator's bundle (FT-092). A `valid` verdict is required (but not sufficient) for admission.

## Out of scope

- Conformance audit (separate decision in FT-092 / ADR-060).
- SBOM scanning (slice 3+).
- Identity rotation (deferred).

## Success criteria

- For each of the five verdict classes, an end-to-end test produces the expected verdict given the corresponding input conditions.
- The verdict artifact has mechanical provenance pointing to the action session, and motivational provenance pointing to the originating `WorkerImageSubmission`.
- The Curator's bundle (FT-092) includes the verdict; the absence of a verdict blocks bundle assembly.
