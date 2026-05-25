---
id: ADR-043
title: Full-chain traversal as a first-class QueryTemplate artifact
status: proposed
features:
- FT-075
supersedes: []
superseded-by: []
domains:
- api
- data-model
scope: cross-cutting
---

## Context

The full-chain traversal — walk any artifact backward to terminal origins (BoundaryArtifacts, sensing-action outputs, initial-request artifacts) and forward to terminal value actions — is the query that makes the audit principle ("did this role have the context it needed?") operationally tractable. Multiple consumers need it:

- **Audit role** (slice 2+) — runs the full chain on disputed verifier verdicts to reconstruct what the implementer saw and where it came from.
- **Meta-loop aggregation** — derives fitness signals from the shape of provenance chains (e.g. average chain depth from Feature to Brief, fraction of artifacts whose chain terminates at a BoundaryArtifact vs. at a synthetic backfill).
- **Human inspection tooling** — `dec session log <id>`, visualization, ad-hoc SPARQL exploration.
- **Fitness functions** — orphan detection, dead-edge detection, dual-provenance compliance audits.

Implementation options:

1. **Each consumer writes its own SPARQL.** Inconsistency across consumers; duplicate maintenance; one consumer fixes a bug, others stay broken.
2. **Hardcoded query in the orchestrator codebase.** Centralized but tied to code; not itself an artifact; cannot be evolved through the framework's normal decision flow.
3. **First-class `QueryTemplate` artifact.** Versioned, has provenance, evolves through the same author-decision-implement flow as any other artifact. Consumers reference by ID.

## Decision

**Define `QueryTemplate` as a first-class artifact type. The full-chain traversal ships as the canonical instance: `qt:full-chain-backward-v1` and `qt:full-chain-forward-v1`.**

```turtle
:QueryTemplateShape a sh:NodeShape ;
  sh:targetClass :QueryTemplate ;
  sh:and ( :MechanicalProvenanceShape ) ;
  sh:property [
    sh:path :querySpec ;
    sh:minCount 1 ; sh:maxCount 1 ;
    sh:datatype xsd:string                                      # SPARQL source
  ] ;
  sh:property [
    sh:path :queryLanguage ;
    sh:minCount 1 ; sh:maxCount 1 ;
    sh:in ( "SPARQL-1.1" )                                      # extensible later
  ] ;
  sh:property [
    sh:path :version ;
    sh:minCount 1 ; sh:maxCount 1 ;
    sh:datatype xsd:string                                      # semver
  ] ;
  sh:or (
    [ sh:property [ sh:path :decomposesFrom ; sh:minCount 1 ; sh:class :Brief ] ]
    [ sh:property [ sh:path :addresses      ; sh:minCount 1 ; sh:class :Feedback ] ]
    [ a sh:NodeShape ; sh:class :BoundaryArtifact ]
  ) .
```

### The slice-1 backward template

Walks `wasGeneratedBy → Session, used → Artifact` (mechanical lineage) unioned with `wasDerivedFrom*` (motivational lineage, transitive via the ADR-039 subPropertyOf relationship). Terminal condition: the ancestor is a `BoundaryArtifact` subclass or has no further `wasDerivedFrom` edges.

```sparql
PREFIX prov: <http://www.w3.org/ns/prov#>
PREFIX dec:  <https://decision-cli.dev/ns#>

# Walk backward from focal artifact :X to terminal origins.
SELECT ?ancestor ?ancestor_type ?via WHERE {
  {
    :X (prov:wasGeneratedBy/prov:used)* ?ancestor .
    BIND("mechanical" AS ?via)
  } UNION {
    :X (prov:wasDerivedFrom)* ?ancestor .                       # motivational; subPropertyOf above
    BIND("motivational" AS ?via)
  }
  ?ancestor a ?ancestor_type .
  FILTER (
    ?ancestor_type IN (dec:BoundaryArtifact, dec:SensingActionOutput, dec:InitialRequest, dec:BootstrapSession)
    || NOT EXISTS { ?ancestor prov:wasDerivedFrom ?_ }
  )
}
```

### The slice-1 forward template

Symmetric: walks the inverse predicates (`^prov:wasGeneratedBy` and `^prov:wasDerivedFrom*`) to find terminal value-action artifacts. Same artifact type, separate `QueryTemplate` instance.

### Why first-class artifacts and not constants

- **Versioning.** When the discipline grows (new artifact types, new predicate categories, new terminal conditions), the query template is updated as a normal decision — author an ADR, ship a new version (`qt:full-chain-backward-v2`), consumers can migrate at their own pace because v1 is still queryable.
- **Provenance for the queries themselves.** Each `QueryTemplate` carries dual provenance per ADR-038, so the audit principle applies to the audit tooling — every consumer of a `QueryTemplate` can ask "what justifies this query?"
- **Decoupled from code.** A query change does not require a `dec` release; the catalog mutation lands and consumers pick it up on next read.
- **Reusable type.** Other reusable queries (bundle assembly queries, audit queries, meta-loop aggregations) are also `QueryTemplate` instances. This ADR introduces the type; other ADRs / features consume it.

### Reference, don't re-derive

Consumers reference queries by ID:

```rust
let qt = store.fetch_query_template("qt:full-chain-backward-v1")?;
let bindings = store.execute(&qt.spec, &[("X", focal_artifact_iri)])?;
```

Not by re-writing the SPARQL string in their own source. The fitness check for "consumers reference, do not re-derive" is a slice-2+ static-analysis check; for slice 1, code review carries it.

### Alternatives considered

- **Library-level constant in Rust + Python.** Centralized but not graph-resident; cannot be queried, audited, or versioned via the discipline; consumers in other languages re-derive.
- **Stored procedure in Oxigraph.** Oxigraph does not support stored procedures.
- **`QueryTemplate` as artifact (adopted).** First-class, versioned, queryable, conforms to the discipline.

## Consequences

**Positive.**

- The full-chain query is single-sourced. Bug fixes propagate to every consumer; vocabulary growth flows in via the ADR-039 subPropertyOf relationship without any template change.
- `QueryTemplate` as a type pays for itself once a second reusable query (the audit role's "did the implementer see this Feedback?" check, for example) lands.
- Versioning is unambiguous — consumers pin to `qt:full-chain-backward-v1`; revisions are explicit migrations.

**Negative / accepted costs.**

- Adds one new artifact type, with shape, validator, and tooling overhead.
- Consumers pay a graph-fetch on each query execution (negligible — the template is loaded once per session and cached).
- Performance tuning of the template (Oxigraph query plan, path-expansion cost) is a slice-2+ concern; slice 1 ships a working query, not an optimal one.

**Boundary enforcement.** `QueryTemplate` instances conform to ADR-038 (dual provenance), so the audit-of-the-audit-tooling property holds recursively. Slice 1 ships the type plus the two canonical instances; the fitness check that consumers reference rather than re-derive is deferred.

## Relationship to existing ADRs

- **ADR-038 / ADR-039.** Consumes — the full-chain query depends on the dual-provenance discipline and the subPropertyOf walkability.
- **ADR-040 (BoundaryArtifact).** Consumes — terminal condition uses BoundaryArtifact membership.
- **ADR-002 (Graph-as-state).** Compatible — `QueryTemplate` is graph state.

## Status

Proposed. Implementation in FT-075. The slice-1 two instances ship with the type; consumer migration to reference-by-ID is a per-consumer story tracked in their own feature_specs.
