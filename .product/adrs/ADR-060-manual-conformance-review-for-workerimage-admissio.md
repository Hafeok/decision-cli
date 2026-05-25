---
id: ADR-060
title: Manual conformance review for WorkerImage admission in slice 1
status: accepted
features:
- FT-092
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:bc0e0abefc34fd2d75b387d249d268d1bb53e615ffbb7990482a7b415cbff508
---

## Context

Conformance audit — replaying a candidate WorkerImage against a corpus of historical bundles with known-good artifacts — is the gold standard for verifying a worker actually does what its labels claim. Slice 1 cannot run automated audit because no corpus exists yet: the system hasn't run long enough to accumulate historical bundles, and no human-curated reference set exists either.

Without any audit mechanism, admission would be unverifiable and the catalog would be vacuous. With a deferred audit mechanism, admission would block until slice 2.

## Decision

Slice 1 uses manual Curator review. The WorkerCurator (human-filled at Level 0 autonomy):

1. Reads the WorkerImageSubmission and the identity-verification verdict.
2. Inspects the source repo and the SBOM reference.
3. Optionally pulls the candidate image and runs it against ad-hoc inputs.
4. Produces a verdict based on judgment.

The verdict materialises as a `ConformanceAudit` artifact of class `manual-review` carrying the Curator's notes, linked from the admitted `WorkerImage`'s `conformance_audits` field.

Slice 2+ adds an `automated-replay` class on the same `ConformanceAudit` artifact: the shape doesn't change, only the audit mechanism. The `class` discriminator lets future queries distinguish the two evidence kinds without schema churn.

## Consequences

- **Positive:** Admission discipline ships in slice 1 without waiting for the corpus.
- **Positive:** The audit artifact shape is stable across slices; corpus accumulation can begin immediately.
- **Negative:** Manual review doesn't scale and is judgment-bound. Acceptable while submissions are rare (single-tenant, slow rate); breaks under high-throughput admission.
- **Negative:** Curator workload is real but bounded — every submission, not every dispatch.

## Alternatives considered

- **Defer admission entirely to slice 2.** Rejected — no admitted workers means no dispatch at all.
- **Auto-admit every submission.** Rejected — capability-tag claims are unverified, label fraud is undetected.

## References

- `brief:worker-distribution-slice-1`
- `feature:automated-conformance-replay` (excluded; slice 2+).
