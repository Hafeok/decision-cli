---
id: ADR-055
title: Worker image artifact type mirrors the Model catalog shape
status: proposed
features:
- FT-086
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
---

## Context

The orchestration catalog already solves the registration / identity-versioning / eligibility / capability-tag-routing problem for LLM models (impl doc §9): a catalog of identity-versioned entries with capability tags, eligibility status, and provenance from registration evidence; policy binds capability tags to specific catalog entries; new entries enter via a registration audit.

WorkerImages have the same structural shape: identity-versioned executables that claim capability tags, accumulate registration evidence, gain or lose eligibility, and bind to policy-selected tags. The question is whether to invent a new shape for them or reuse the Model shape.

## Decision

Mirror the Model catalog shape for `WorkerImage`. Same field vocabulary (identity, version, capability tags, eligibility status, provenance), same registration discipline (audit → admit → bind), same policy mechanism (capability-tag-to-entry binding).

`WorkerImage` carries: `id`, `name`, `version`, `registry_ref` (OCI reference with digest), `capability_tags`, `compatible_roles`, `signed_by` (sigstore Fulcio identity), `sbom_ref` (OCI referrer URI), `conformance_audits`, `eligibility_status` (qualified | candidate | deprecated | pulled), `provenance` (source repo URI, commit hash, GitHub Actions run URL), plus mechanical and motivational provenance per dual-provenance discipline.

## Consequences

- **Positive:** Avoids reinventing concepts the framework already has language for. Reduces the number of distinct mental models the user has to hold.
- **Positive:** Opens the door to a future cross-cutting `Eligible` abstract supertype shared by `Model` and `WorkerImage`, and to aggregate queries across both catalogs.
- **Positive:** Policy primitives (capability-tag-to-entry binding) work unchanged.
- **Negative:** Couples future schema evolution: if Model and WorkerImage need to diverge, the shared vocabulary becomes a constraint. Acceptable — the framework explicitly favours convergent shapes for analogous problems.

## Alternatives considered

- **Invent a parallel but distinct shape for WorkerImage.** Rejected — duplicates concepts, multiplies SHACL surface, prevents shared queries.
- **Subsume WorkerImage under Model with a `kind` discriminator.** Rejected — Model and WorkerImage are not the same kind of thing (one is a remote API endpoint, the other an executable artifact); the catalog shape is the right level of reuse, not the type itself.

## References

- `brief:worker-distribution-slice-1`
- `docs/ddd/Implementing_DDD.md` §9 — the Model catalog this mirrors.
