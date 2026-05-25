---
id: ADR-039
title: Motivational predicates as rdfs:subPropertyOf prov:wasDerivedFrom
status: accepted
features:
- FT-070
supersedes: []
superseded-by: []
domains:
- data-model
scope: cross-cutting
content-hash: sha256:f113500b448087db0b6e96992cf9ca417e54ed12252fee8a41a9cf857d3f95f4
---

## Context

ADR-038 establishes the dual-provenance discipline: every artifact carries a mechanical PROV-O block plus at least one motivational predicate edge. The motivational predicate vocabulary is per-type (`addresses`, `decomposes_from`, `originated_from`, `decides_for`, `validates`, `responds_to`, `audits`, …; see FT-070 for the slice-1 catalog).

Consumers of the provenance graph fall into two camps:

1. **Type-specific traversal** — "find every Feature that `addresses` a Feedback of class `audit_fail`". These queries already know which predicate they care about.
2. **Generic traversal** — the full-chain query (FT-075 / ADR-043), the audit role, the meta-loop's fitness functions. These need to walk *all* motivational edges uniformly regardless of which specific predicate they instantiate.

If every motivational predicate is its own top-level relation, generic traversal must enumerate the entire vocabulary in a SPARQL `UNION` block or `(p1|p2|p3|…)*` property path. Every time the vocabulary grows, every generic-traversal consumer breaks (silently — the query returns less than it should, and there is no signal that the result is incomplete).

## Decision

**Every motivational predicate is declared as `rdfs:subPropertyOf prov:wasDerivedFrom` in the shape files.**

```turtle
:addresses        rdfs:subPropertyOf prov:wasDerivedFrom .
:decomposesFrom   rdfs:subPropertyOf prov:wasDerivedFrom .
:originatedFrom   rdfs:subPropertyOf prov:wasDerivedFrom .
:decidesFor       rdfs:subPropertyOf prov:wasDerivedFrom .
:validates        rdfs:subPropertyOf prov:wasDerivedFrom .
:respondsTo       rdfs:subPropertyOf prov:wasDerivedFrom .
:audits           rdfs:subPropertyOf prov:wasDerivedFrom .
# … one declaration per predicate, central in motivational-predicates.ttl
```

Consequences for queries:

- **Generic traversal** walks `prov:wasDerivedFrom*` and gets every motivational ancestor automatically. Vocabulary growth is transparent — new predicates are subProperties, so they flow into the same traversal without query changes.
- **Predicate-specific traversal** keeps working: `?x :addresses ?feedback` still returns exactly the `:addresses` edges.

### Reasoning materialization vs query-time expansion

`rdfs:subPropertyOf` reasoning can be applied two ways:

- **Materialize at write time.** Every `:addresses` triple causes a `prov:wasDerivedFrom` triple to also be asserted in the graph. Queries are cheap; storage grows; reasoning is committed.
- **Apply at query time.** The SPARQL engine expands subProperty paths during query evaluation. Storage is unchanged; queries cost more.

**Slice-1 decision: query-time expansion via Oxigraph's property-path operators.** Oxigraph supports `prov:wasDerivedFrom+` (transitive closure) and SHACL property-shape declarations natively; no materialization step is needed for the slice-1 use cases. If full-chain queries become a hot path (meta-loop running them on every dispatch, for instance), revisit and materialize.

### Alternatives considered

- **No subPropertyOf relationship; consumers enumerate the vocabulary.** Forces every generic traversal to know the full predicate list. Every vocabulary addition silently breaks every consumer that didn't update its query. Rejected as fragile.
- **A single `:motivatedBy` predicate with the specific relationship as a qualified-relation entity (PROV's `qualifiedDerivation` pattern).** More expressive, much more verbose. Adds an entire qualified-relation tier that nothing in the slice-1 use cases needs. Rejected for slice 1; revisit if richer motivational metadata (timestamp of when the relationship was asserted, who asserted it, etc.) becomes load-bearing.
- **Use `prov:wasInfluencedBy` (the most general PROV relation) as the parent.** Too broad — `wasInfluencedBy` also covers communication and delegation relationships that aren't motivational. `wasDerivedFrom` is the precise PROV concept: "an entity derived from another entity." Matches the motivational semantics exactly.

## Consequences

**Positive.**

- One predicate (`prov:wasDerivedFrom`) suffices for every uniform-traversal use case. Audit, meta-loop, visualization — same predicate everywhere.
- Vocabulary growth is non-breaking by construction.
- PROV-O-aware tooling (visualizers, federated stores, external auditors) gets uniform motivational lineage for free, because we are speaking standard PROV.

**Negative / accepted costs.**

- Query-time path expansion has cost; for a graph the size of decision-cli's it is negligible, but at very large scale the materialization tradeoff revisit is required.
- Consumers that want to know *which* motivational predicate was used (for richer narratives) must use the specific predicate name, not the generic `prov:wasDerivedFrom`. Both are available; the generic traversal is purely additive.

**Boundary enforcement.** The subPropertyOf declarations live in `shapes/motivational-predicates.ttl` (FT-072). GraphWriter's SHACL validation enforces type-specific shape constraints; the subPropertyOf reasoning is a query-time concern handled by Oxigraph's path operators.

## Status

Proposed. Implementation lives in FT-070 (vocabulary definition) and FT-072 (shape file layout). The full-chain query template (FT-075 / ADR-043) is the primary slice-1 consumer of the subPropertyOf relationship.
