---
id: ADR-058
title: Cosign keyless signing via GitHub OIDC from slice 1
status: accepted
features:
- FT-089
- FT-106
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:edc60803c265f0df178b7cc8f790faa8974addeb6cac2ec3dbeab279c52c3dcb
---

## Context

The original plan deferred Fulcio + Rekor keyless signing to slice 2 under the assumption that "no CI is in the picture yet" — local key-based signing would be temporarily simpler. With GitHub Actions chosen as the slice-1 release driver (`adr:reusable-workflow-vs-per-repo` / ADR-061), CI is in the picture from day one and the simplification of local keys no longer pays off. Keyless signing via GitHub's ambient OIDC token is exactly as little code as a local key flow, with much stronger properties (no key material in repos, identity tied to the workflow run).

## Decision

Worker releases sign images keyless using `cosign sign` with the ambient GitHub OIDC token. The signing identity is the Fulcio-issued certificate's subject (the GitHub Actions workflow run identity: repo + workflow path + ref). The Rekor transparency log entry is referenced from the WorkerImageSubmission so the verifier can confirm both signature validity and log inclusion.

The orchestration system keeps a trust list of permitted Fulcio identities, matched by GitHub repo, workflow path, and tag pattern. Only signatures from listed identities are valid.

Local key-based signing remains a supported fallback for development workflows that don't run through GitHub Actions, but is not the primary path. A submission signed by a local key is admissible only if the local-key identity has been explicitly enrolled in the trust list.

## Consequences

- **Positive:** Zero secret key material in worker repos.
- **Positive:** The signing identity is the workflow run, so revoking a compromised workflow path (or restricting which refs can release) is a trust-list edit.
- **Positive:** Rekor inclusion gives a public audit trail of every signed release.
- **Negative:** Hard dependency on Fulcio + Rekor availability for releases. Both are operated by the OpenSSF / Sigstore project with strong SLA; an offline signing mode exists for emergencies. Acceptable.
- **Negative:** Identity rotation (compromised signing identity) requires a trust-list policy. Tracked under the brief's open questions; deferred until a real rotation event happens.

## Alternatives considered

- **Local key-based signing** as the primary path. Rejected — requires storing key material somewhere, weaker provenance.
- **Defer all signing to slice 2.** Rejected — admission discipline requires a verifiable signing identity from day one; running unsigned workers makes the catalog vacuous.

## References

- `brief:worker-distribution-slice-1`
- ADR-061 (Reusable workflow over per-repo — the slice-1 release driver).
