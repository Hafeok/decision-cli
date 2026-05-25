---
id: FT-075
title: 'decision-cli: Full-chain provenance SPARQL query as a QueryTemplate artifact'
phase: 3
status: planned
depends-on:
- FT-069
- FT-070
- FT-071
adrs:
- ADR-043
tests:
- TC-125
domains:
- api
- data-model
domains-acknowledged: {}
---

## Description

Define `QueryTemplate` as a first-class artifact type and ship the canonical full-chain SPARQL traversals — `qt:full-chain-backward-v1` (artifact → terminal origins) and `qt:full-chain-forward-v1` (artifact → terminal value actions) — as the slice-1 instances (ADR-043).

The full-chain traversal walks any focal artifact backward through mechanical provenance (`wasGeneratedBy → Session, used → Artifact`) and motivational provenance (`wasDerivedFrom*`, walkable uniformly thanks to ADR-039's subPropertyOf relationship) to terminal origins (BoundaryArtifact subclasses or nodes with no further derivation). It is the query that makes the audit principle ("did this role have the context it needed?") operationally tractable for every consumer that needs it.

QueryTemplate is itself a graph-resident artifact, conforming to the dual-provenance discipline like every other type — versioned, queryable, evolvable via the framework's normal author-decision-implement flow.

## Functional Specification

### Inputs

- The dual-provenance shape set (FT-072) — `QueryTemplateShape` is added here.
- The motivational-predicate vocabulary (FT-070) — the subPropertyOf relationship is what makes generic traversal possible.
- The BoundaryArtifact class (FT-071) — the terminal condition for backward traversal.
- Oxigraph's SPARQL 1.1 implementation including property-path operators (`*`, `+`, `/`, `|`).

### Outputs

- `crates/decision-cli/src/core/ontology/shapes/query-template.ttl` — the `QueryTemplate` class and `QueryTemplateShape` SHACL shape.
- `crates/decision-cli/src/core/queries/full_chain.rs` — the Rust accessor module exposing `QueryTemplate` fetch + execute helpers.
- Two TTL fixtures shipped as catalog-bootstrap data (extending FT-009):
  - `bootstrap/qt-full-chain-backward-v1.ttl` — the canonical backward query as a `QueryTemplate` instance.
  - `bootstrap/qt-full-chain-forward-v1.ttl` — the canonical forward query.
- A new CLI subcommand `dec query template list` / `dec query template show <id>` for inspection.
- A reference helper for consumers:
  ```rust
  pub fn fetch_full_chain_backward(&self) -> Result<QueryTemplate, StoreError>;
  pub fn execute(&self, qt: &QueryTemplate, bindings: &[(&str, &str)]) -> Result<QueryResults, StoreError>;
  ```

### State

- Two `QueryTemplate` instances live in the orchestration store after `dec init`, seeded by the catalog bootstrap path (FT-009 extension).
- The TTL fixtures are the source of truth; the store-resident copies are written once at bootstrap and updated only by an explicit re-bootstrap or version bump.
- No per-execution state — the helpers fetch and execute on demand.

### Behaviour

1. **Type and shape** — `query-template.ttl`:

   ```turtle
   @prefix sh:   <http://www.w3.org/ns/shacl#> .
   @prefix xsd:  <http://www.w3.org/2001/XMLSchema#> .
   @prefix dec:  <https://decision-cli.dev/ns#> .

   dec:QueryTemplate a rdfs:Class ;
       rdfs:label "QueryTemplate" ;
       rdfs:comment "Versioned, graph-resident SPARQL template referenced by ID." .

   dec:QueryTemplateShape a sh:NodeShape ;
       sh:targetClass dec:QueryTemplate ;
       sh:and ( dec:MechanicalProvenanceShape ) ;
       sh:property [ sh:path dec:querySpec     ; sh:minCount 1 ; sh:maxCount 1 ; sh:datatype xsd:string ] ;
       sh:property [ sh:path dec:queryLanguage ; sh:minCount 1 ; sh:maxCount 1 ; sh:in ( "SPARQL-1.1" ) ] ;
       sh:property [ sh:path dec:version       ; sh:minCount 1 ; sh:maxCount 1 ; sh:datatype xsd:string ] ;
       sh:or (
           [ a sh:NodeShape ; sh:class dec:BoundaryArtifact ]
           [ sh:property [ sh:path dec:decomposesFrom ; sh:minCount 1 ; sh:class dec:Brief    ] ]
           [ sh:property [ sh:path dec:addresses      ; sh:minCount 1 ; sh:class dec:Feedback ] ]
       ) .
   ```

2. **The slice-1 backward template** (shipped in `bootstrap/qt-full-chain-backward-v1.ttl`):

   ```sparql
   PREFIX prov: <http://www.w3.org/ns/prov#>
   PREFIX dec:  <https://decision-cli.dev/ns#>
   PREFIX rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

   SELECT ?ancestor ?ancestor_type ?via WHERE {
       {
           ?focal (prov:wasGeneratedBy/prov:used)* ?ancestor .
           BIND("mechanical" AS ?via)
       } UNION {
           ?focal (prov:wasDerivedFrom)* ?ancestor .       # subPropertyOf walks all motivational predicates
           BIND("motivational" AS ?via)
       }
       ?ancestor rdf:type ?ancestor_type .
       FILTER (
           EXISTS { ?ancestor rdf:type/rdfs:subClassOf* dec:BoundaryArtifact }
           || NOT EXISTS { ?ancestor prov:wasDerivedFrom ?_ }
       )
   }
   ```

   The placeholder `?focal` is bound at execution time via the `bindings` parameter on the helper.

3. **The slice-1 forward template** — symmetric, traversing the inverse property paths (`^prov:wasGeneratedBy / ^prov:used` and `^prov:wasDerivedFrom`).

4. **Helper API** — Rust:

   ```rust
   let store = OrchestrationStore::open(path)?;
   let qt = store.fetch_query_template("qt:full-chain-backward-v1")?;
   let results = store.execute_template(&qt, &[("focal", focal_iri)])?;
   for row in results.rows() {
       println!("{}: {} via {}", row["ancestor"], row["ancestor_type"], row["via"]);
   }
   ```

5. **CLI surface** — `dec query template list` enumerates registered templates; `dec query template show <id>` prints the spec, version, and provenance. (Execution wraps separately in audit-tooling features — slice 1 ships only the inspection commands.)

6. **Versioning** — the slice-1 instances have `version: "1.0.0"`. Future revisions ship as new `QueryTemplate` instances (`qt:full-chain-backward-v2`); consumers pin to a specific version. Deprecation of an old version follows the framework's standard supersession flow.

### Invariants

- **Templates are referenced, never re-derived.** Consumers fetch by ID and execute; they do not embed the SPARQL string. A slice-2+ static-analysis fitness check enforces this; slice 1 carries it via code review.
- **Subclass-aware terminal condition.** The backward query's FILTER uses `rdf:type/rdfs:subClassOf*` to match every `BoundaryArtifact` subclass. Validator subclass reasoning (FT-073) makes this work.
- **`querySpec` is the SPARQL source verbatim.** No template substitution at storage; binding-substitution happens at execute time via the SPARQL engine's query-with-bindings API.
- **Template provenance conforms.** The two slice-1 instances are `BoundaryArtifact` boundary-class members (their motivational origin is the `brief:dual-provenance-discipline` Brief, which is itself a boundary artifact); when the migration runs, the Brief is re-authored as a Brief artifact and the templates can declare `decomposes_from` to it instead. Until then, boundary membership satisfies the shape.
- **Path-expansion cost is the slice-1 performance budget.** A full-chain query against a 1000-artifact graph completes in < 100 ms p99. Larger graphs may require materialised subPropertyOf reasoning; revisit per ADR-039's tradeoff.

### Error handling

- `StoreError::TemplateNotFound` if a consumer requests an unknown template ID.
- `StoreError::QueryExecution` wrapping oxigraph SPARQL errors; consumer-facing message names the template and the unbound variable.
- `StoreError::ShapeViolation` if a manually-authored `QueryTemplate` violates the shape at write time — handled by the standard FT-073 validator path.

### Boundaries

- **In scope.** The `QueryTemplate` type + shape. The two slice-1 instances (backward + forward). The Rust helper API. The `dec query template list/show` CLI surface. Catalog-bootstrap extension to seed the instances. A TC asserting the backward template returns the expected ancestor set on a fixture graph.
- **Out of scope.** Audit tooling that consumes the template (slice 2+). Visualisation of full chains (much later). Materialised subPropertyOf reasoning (ADR-039 tradeoff revisit). Federated-graph traversal (Brief open question 3 + worker-distribution Brief; later slice).

## Out of scope

- A general "query catalog" UX. `dec query template` is two read-only subcommands; richer query management is future work.
- Bundle-assembly queries as QueryTemplate instances. SDK-Brief's `feature:shape-codegen` is the future consumer; this feature ships the type, not its other instances.
- Template-execution caching. Slice 1 executes on every call.

## References

- [ADR-043](ADR-043) — Full-chain traversal as first-class QueryTemplate (the decision this feature implements).
- [ADR-039](ADR-039) — Motivational subPropertyOf (what makes generic traversal possible).
- [ADR-040](ADR-040) — BoundaryArtifact (the terminal condition).
- [FT-069](FT-069), [FT-070](FT-070), [FT-071](FT-071) — Provenance primitives the query walks over.
- [FT-009](FT-009) — Catalog-bootstrap path this feature extends.
