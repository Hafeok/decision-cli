---
id: FT-089
title: 'worker-distribution: Cosign keyless signing flow via GitHub OIDC'
phase: 3
status: planned
depends-on: []
adrs:
- ADR-058
tests: []
domains: []
domains-acknowledged: {}
---

## Motivation

Derived from `brief:worker-distribution-slice-1`. Catalog admission requires a verifiable signing identity per WorkerImage. Addresses ADR-058 (cosign keyless via GitHub OIDC).

## Scope

- Worker release workflow uses `cosign sign --keyless` with the ambient GitHub OIDC token. No private key material in repos.
- The signing identity (Fulcio-issued certificate subject and issuer) is captured on the `WorkerImageSubmission` (`signature_identity` field) so admission can verify.
- The Rekor transparency log entry produced by cosign is referenced from the Submission so the verifier (FT-090) can confirm both signature validity and log inclusion.
- A trust list of permitted Fulcio identities is maintained inside the orchestration system, matched by:
  - GitHub repo (owner/name)
  - Workflow path (`.github/workflows/release.yml` or a specific reusable workflow ref)
  - Tag pattern (e.g. `implementer-v*.*.*`)
  - Only signatures whose identity matches a listed entry are valid.
- Local key-based signing remains supported as a fallback (development workflows outside GitHub Actions). A local-key identity is admissible only if explicitly enrolled in the trust list.

## Out of scope

- Identity rotation policy when a signing identity is compromised (deferred; tracked in brief open questions).
- Cross-CI portability (slice 3+ concern; the wire-level primitives — OIDC, cosign, Rekor — are portable, only the workflow file is GitHub-specific).
- Automatic trust-list management (slice 1: edited manually by the operator).

## Success criteria

- A release workflow signs an image keyless and the resulting Submission carries Fulcio identity + Rekor entry pointers that the verifier resolves and accepts.
- A signature from an identity NOT on the trust list is rejected at verification time with verdict `untrusted-identity`.
- A submission claiming a Rekor entry that doesn't exist (or doesn't match) is rejected with verdict `rekor-entry-missing`.
