---
id: ADR-027
title: Authority declarations in the role catalog
status: proposed
features: []
supersedes: []
superseded-by: []
domains: []
scope: cross-cutting
source-files:
- crates/decision-cli/src/core/role_catalog/seeds/implementer-authority.ttl
- crates/decision-cli/src/core/role_catalog/seeds/verifier-authority.ttl
---

## Context

The framework's central claim — decisions and actions compose into self-correcting chains — depends on roles having *bounded authority*. A role that may unilaterally amend an ADR is not a role; it is a god-mode actor and the audit trail evaporates.

Slice 1 sidesteps this: there is only one role (implementer), it produces a single artifact type (`CodeChange`), the implicit authority is "write code, don't write specs." That implicit boundary survives slice 1 only because the surface is tiny.

Slice 3 introduces feedback flow. Feedback emission *is* a role's claim about what is outside its authority ("the spec author should fix this gap, not me"). For that claim to be coherent, every role's authority must be declared, machine-readable, and consumed by the worker as part of its bundle.

## Decision

**Every role in the role catalog carries an `authority` declaration in its catalog entry. The declaration enumerates what judgments are in-scope and what requires escalation via feedback. Worker bundles include the declaration verbatim; workers consume it via the dispatch event payload.**

### Authority declaration shape

A role's catalog entry (graph artifact, type `dec:Role`) gains:

```turtle
@prefix dec: <https://decision-cli.dev/ns#> .

<role:implementer> a dec:Role ;
    dec:name "implementer" ;
    dec:authority [
        a dec:Authority ;
        dec:mayDecide ( "code-style"
                        "naming-within-feature"
                        "internal-data-shape"
                        "test-cases-for-this-feature" ) ;
        dec:mustEscalate ( "feature-spec-changes"
                           "adr-changes"
                           "new-artifact-types"
                           "cross-cutting-policy" ) ;
        dec:escalateVia [
            dec:className "gap" ;            # default for under-specification
            dec:targetRole "spec-author" ;
        ], [
            dec:className "contradiction" ;
            dec:targetRole "architect" ;
        ], [
            dec:className "capability-request" ;
            dec:targetRole "architect" ;
        ] ;
        dec:rationale "Implementer writes code from a feature_spec; structural changes to the spec or to ADRs go through their authoring roles. Implementer self-decides anything that does not survive the file boundary of the feature." ;
    ] .
```

### Field semantics

| Field | Meaning |
|---|---|
| `dec:mayDecide` | A controlled list of categories the role's worker may resolve unilaterally. The list is the worker's standing instructions: "any judgment falling under one of these names, decide and proceed." |
| `dec:mustEscalate` | A controlled list of categories where the worker MUST emit feedback rather than decide. Worker prompts include this list verbatim. |
| `dec:escalateVia` | Repeating: a (class, target-role) hint per escalation category. Maps directly to [ADR-023](ADR-023) classes and [ADR-026](ADR-026) routing. |
| `dec:rationale` | One-paragraph explanation. Read by humans, ignored by workers. |

### Where the categories come from

Categories are a controlled vocabulary, parallel to but distinct from [ADR-023](ADR-023)'s feedback classes. A category names a *kind of decision*; a class names a *kind of feedback emission*. Many-to-one: multiple categories may share a class (e.g. `feature-spec-changes` and `new-artifact-types` both escalate via `gap`).

The initial category set (extensible by ADR amendment, same shape as the feedback-class vocabulary):

`code-style`, `naming-within-feature`, `internal-data-shape`, `test-cases-for-this-feature`, `feature-spec-changes`, `adr-changes`, `new-artifact-types`, `cross-cutting-policy`, `role-catalog-changes`, `routing-table-changes`, `policy-changes`.

### How workers consume it

The bundle assembler ([FT-011](FT-011)-style for slice 1, generalized in slice 2/3) injects the role's authority declaration into the dispatch event payload. Worker SDKs ([FT-031](FT-031)) expose `bundle.authority` as a structured object. Worker system prompts include:

> Your authority: you may decide on any of {`mayDecide` list}. You MUST emit feedback (do not decide) on any of {`mustEscalate` list}. When in doubt, emit feedback.

Worker behavior with respect to authority is itself measurable: an emitting worker that emits feedback for an *in-scope* category is over-cautious (signal); a worker that decides on an *out-of-scope* category and produces an artifact is overstepping (signal — Phase C input).

### Why declared and not implicit

Three reasons:

1. **The boundary is the contract.** A role without a declared authority has implicit authority — "whatever the prompt happens to encourage." That is the same shape as no role at all.
2. **Cross-role coherence.** When the verifier and the implementer disagree about whether the implementer was authorized to make a change, the authority declaration is the arbiter. Without a declaration, the disagreement is unresolvable.
3. **Phase C measurement.** Aggregating feedback-emission patterns by category requires the categories to exist as graph identifiers, not as free-form prompt text.

### How authority interacts with feedback

[ADR-022](ADR-022) — [ADR-026](ADR-026) define how feedback flows. The authority declaration defines *when* a feedback emission is correct:

- A worker hitting a `mustEscalate` category MUST emit feedback (typically blocking, per the class default).
- A worker hitting a `mayDecide` category MAY decide. If it chooses to emit feedback anyway (non-blocking `defect` or `capability-request`), that's allowed but recorded.
- A worker producing an artifact that resolves a `mustEscalate` category without first emitting feedback is a violation — caught by slice-3 invariant TCs against the dispatch's session telemetry.

### Where the declarations live in the SDP

The role catalog is `core/` data; per the slice-level SDP convention in `CLAUDE.md`, the authority schema and the per-role declarations live under `core/ontology/` and `core/role_catalog/`. Slice-3 features and Phase B role additions consume them; they never modify the catalog by reaching into another feature's module.

## Rejected alternatives

- **Implicit authority via worker system prompts only.** Rejected: opaque, unaggregable, untestable.
- **Authority as a single free-form field.** Rejected: same failure mode as free-form feedback classes — invites prose drift, prevents measurement.
- **Authority embedded in feature_specs instead of role catalog.** Rejected: a role's authority is stable across the features it serves; embedding it in each feature_spec would duplicate it many times and risk drift.
- **No authority declaration; rely on review.** Rejected: review is a manual fitness function. The point of declared authority is to make the boundary mechanical.

## Consequences

**Positive:**
- Workers have a clear, structured contract for "when to decide vs. when to escalate."
- Role boundaries are reviewable as graph artifacts.
- Phase C can measure overstepping and over-caution per role.
- Adding a role (Phase B) is now a well-defined operation: extend the role catalog with an entry that has an authority declaration.

**Negative / accepted costs:**
- Every new role requires deliberate authoring of its authority. This is by design (the boundary IS the role) but it does mean Phase B's first role takes longer to land than a "just write the worker" approach would.
- The category vocabulary is itself a schema artifact subject to drift; amendment discipline applies.

**Enforcement:**
- SHACL shape on `dec:Authority` validates structure at write time.
- A slice-3 TC asserts every role in the catalog has a non-empty authority declaration.
- A cross-cutting TC under [ADR-014](ADR-014) asserts: for every dispatched session, the feedback it emits has classes consistent with the role's `mustEscalate` list (no role escalating outside its declared categories).

## Status

Proposed. Linked to [FT-030](FT-030).
