# Working session: dual-provenance discipline

Authoring format: option-2 (one file per working session, typed sections per
artifact, `@<predicate> <ref>` for edges). Predicate names and ID conventions
remain proposals — adjust to whatever product-cli's catalog actually uses.

This Brief is framework-level. It establishes the universal provenance rule
that every other Brief depends on for SHACL finalization. It is also itself a
boundary artifact in the sense it defines: an initial-request entering the
orchestration system from outside, with no graph-internal motivational origin
because it predates the graph that would track it.

---

## Brief brief:dual-provenance-discipline

title: Establish the dual-provenance discipline as the universal substrate for all artifacts

@references doc:impl-doc-§3                      (RDF + PROV-O substrate already committed)
@references doc:entity-reference-provenance      (existing provenance entry; this Brief extends it)
@references doc:foundations-sensing              (input-boundary origins)
@references doc:foundations-value-anchoring      (output-boundary terminations)
@references brief:pipeline-worker-slice-1         (consumer, blocked on this)
@references brief:worker-distribution-slice-1     (consumer, blocked on this)

@decomposes_into feature:mechanical-provenance-block
@decomposes_into feature:motivational-predicate-vocabulary
@decomposes_into feature:boundary-artifact-class
@decomposes_into feature:provenance-shacl-shapes
@decomposes_into feature:graphwriter-shacl-enforcement
@decomposes_into feature:existing-artifact-migration
@decomposes_into feature:full-chain-query-template

@excludes feature:continuous-orphan-fitness-function   (slice 2+)
@excludes feature:predicate-vocabulary-versioning       (slice 2+)
@excludes feature:federated-cross-system-prov-queries   (depends on artifact bus; later)
@excludes feature:provenance-visualization-ui           (much later)
@excludes feature:provenance-redaction-privacy          (regulatory concern; later)
@excludes feature:retroactive-motivational-inference    (slice 3+ if migration leaves orphans)

@acknowledges ack:bootstrap-self-reference
@acknowledges ack:vocabulary-will-grow
@acknowledges ack:mechanical-provenance-requires-chokepoint
@acknowledges ack:this-brief-is-a-boundary-artifact

premise:
  The framework has committed to PROV-O as the provenance substrate (impl
  doc §3) but has not specified the discipline that governs how every artifact
  type uses it. Without an explicit rule, artifacts will accumulate with
  inconsistent provenance: some carrying full PROV-O, some carrying ad-hoc
  fields, some carrying nothing. The audit principle — "did this role have
  the context a competent human in this role would have" — depends on being
  able to walk back through provenance to read what informed a decision.
  Without uniform discipline that property is best-effort and fragile.

  The SDK Brief (brief:pipeline-worker-slice-1) and the worker-distribution
  Brief (brief:worker-distribution-slice-1) both reference SHACL shapes that
  need this discipline finalized before they can be authored. Authoring this
  Brief is the unblocking move.

goal:
  Establish the universal rule that every artifact has dual provenance:
  mechanical (auto-attached by GraphWriter from the producing session) and
  motivational (declared by the author, per-type controlled vocabulary).
  Enforce both via SHACL at write time. Define the boundary-artifact class
  for sensing-action outputs and initial-request artifacts that legitimately
  carry only mechanical provenance. Migrate product-cli's existing artifact
  types (Feature, ADR, TC, Dependency) to conform. Establish the full-chain
  SPARQL query template as the canonical traversal for audit and meta-loop
  consumers.

  The discipline is framework-level: it touches every artifact type in
  every system that ever joins the platform. Slice 1 of this Brief lands
  the substrate; subsequent slices grow vocabulary and add automated
  fitness functions, but the principle is fixed here.

success_criteria:
  - The mechanical-provenance SHACL block is defined once and imported by
    every artifact type's shape via sh:and composition. Adding a new
    artifact type requires three lines (sh:targetClass, sh:and the
    mechanical block, sh:or the motivational alternatives).
  - The motivational-predicate vocabulary lists, per artifact type, the
    set of permitted origin predicates and their range constraints. At
    least one is required by the type's shape.
  - The BoundaryArtifact class is defined; instances are exempt from the
    motivational-provenance requirement at the type-shape level.
  - GraphWriter enforces both blocks on every write. Rejection produces a
    structured violation report identifying the missing fields, returned
    to the producer.
  - Product-cli's existing Feature, ADR, TC, Dependency artifacts have
    been migrated: their shapes now require dual provenance; any existing
    instances that pass migration audit are grandfathered with backfilled
    mechanical provenance; instances that don't pass are flagged for human
    repair.
  - The full-chain SPARQL query template returns, for any artifact: the
    full chain backwards to its terminal origins (sensing actions or
    initial-request artifacts) and forwards to its terminal value actions.
    Same query shape works on product-cli's graph and pipeline-cli's
    orchestration graph.

---

## Feature feature:mechanical-provenance-block

title: Define the universal mechanical-provenance SHACL fragment

@motivated_by brief:dual-provenance-discipline
@addresses_decision adr:two-flavors-of-provenance

A reusable SHACL NodeShape fragment that every artifact type's shape composes
in via sh:and. The fragment encodes PROV-O mechanical provenance:

```turtle
:MechanicalProvenanceShape a sh:NodeShape ;
  sh:property [
    sh:path prov:wasGeneratedBy ;
    sh:minCount 1 ; sh:maxCount 1 ;
    sh:class :Session
  ] ;
  sh:property [
    sh:path prov:wasAttributedTo ;
    sh:minCount 1 ;
    sh:class :Agent
  ] ;
  sh:property [
    sh:path prov:generatedAtTime ;
    sh:minCount 1 ; sh:maxCount 1 ;
    sh:datatype xsd:dateTime
  ] .
```

Three fields, three semantics:

- `prov:wasGeneratedBy` — the Session that produced this artifact. Single-
  valued; an artifact has exactly one producing session.
- `prov:wasAttributedTo` — the Agent (role + model, or role + human) the
  Session was attributed to. Multi-valued allowed when a session is
  jointly attributed (e.g., human-supervised LLM session).
- `prov:generatedAtTime` — the write timestamp from the GraphWriter
  transaction. Single-valued, monotonically increasing within a named
  graph.

These three fields are populated by GraphWriter from the session record at
write time. Workers do not author them. The role of the SDK's Session layer
(per brief:pipeline-worker-slice-1) is to ensure the session record exists;
GraphWriter materializes the triples from it.

Session itself carries:

```turtle
:SessionProvenanceShape a sh:NodeShape ;
  sh:property [
    sh:path prov:used ;
    sh:nodeKind sh:IRI ;
    sh:class :Artifact
  ] ;
  sh:property [
    sh:path prov:wasInformedBy ;
    sh:nodeKind sh:IRI ;
    sh:class :Session
  ] ;
  sh:property [
    sh:path prov:wasAssociatedWith ;
    sh:minCount 1 ;
    sh:class :Agent
  ] .
```

`prov:used` lists the artifacts in the Session's bundle. `prov:wasInformedBy`
lists prior sessions whose outputs were among those artifacts (transitively
walks the decision DAG backwards through sessions). `prov:wasAssociatedWith`
is the Session's attribution to the Agent that performed it.

Walking `wasGeneratedBy → Session, used → Artifact, recurse` is the full-chain
traversal feature:full-chain-query-template formalizes.

---

## Feature feature:motivational-predicate-vocabulary

title: Catalog motivational predicates per artifact type

@motivated_by brief:dual-provenance-discipline
@addresses_decision adr:motivational-predicates-as-prov-derived-from-subtypes

The slice-1 vocabulary. Each artifact type's SHACL shape requires at least
one of the listed predicates via sh:or composition. Range constraints
encoded per predicate. All motivational predicates are declared as
rdfs:subPropertyOf prov:wasDerivedFrom so the full-chain query treats them
uniformly.

| Artifact type      | Required (at least one of)                                                                                                  |
|--------------------|------------------------------------------------------------------------------------------------------------------------------|
| Feature            | `addresses → Feedback` \| `decomposes_from → Brief` \| `originated_from → DiscoveryFinding` \| `responds_to → Question`     |
| ADR                | `decides_for → Feature` \| `addresses → Question` \| `supersedes → ADR`                                                      |
| TC (TestCriterion) | `validates → Feature` \| `validates → ADR`                                                                                   |
| Dependency         | `required_by → Feature` \| `required_by → ADR`                                                                               |
| Brief              | `responds_to → Request` \| `responds_to → Feedback` \| boundary-artifact class membership                                    |
| Acknowledgement    | `motivated_by → Brief` \| `motivated_by → ADR`                                                                               |
| Feedback           | `observed_in → Session` \| `observed_via → SensingAction` \| `produced_by → Role`                                            |
| DiscoveryFinding   | `derived_from → SensingAction`                                                                                               |
| Question           | `raised_in → Session` \| `raised_by → Brief` \| `raised_by → ADR`                                                            |
| WorkerImage        | `addresses → Feedback` \| `decomposes_from → Brief` \| `originated_from → DiscoveryFinding`                                  |
| ConformanceAudit   | `audits → WorkerImage`                                                                                                       |
| Model              | `decomposes_from → Brief` \| `addresses → CapabilityGap`                                                                     |
| Policy             | `decomposes_from → Brief` \| `addresses → Feedback`                                                                          |
| WorkerImageSubmission | boundary-artifact class membership                                                                                       |
| Session            | (carries its own mechanical block via SessionProvenanceShape; not a derived artifact)                                       |
| Subscription       | `motivated_by → Brief` \| `motivated_by → Policy`                                                                            |
| Dispatch           | (carries its own mechanical block; dispatches are session-generated artifacts)                                              |

Notes on selected entries:

- **Feature** picks up `responds_to → Question` for the case where a Feature
  resolves an OpenQuestion raised in an earlier session. This is the path
  open questions take from narrative to first-class artifact.
- **Brief** can satisfy the requirement via boundary-artifact class
  membership: a Brief that originates from outside the orchestration graph
  (like this one) has no graph-internal motivational origin and declares
  itself a boundary artifact instead.
- **ADR's `supersedes → ADR`** captures the case where one ADR's existence
  is motivated by revising another. The full chain walks back through the
  supersession chain to the original-motivated ADR.
- **WorkerImageSubmission** is always a boundary artifact (entering from
  CI), explicitly not requiring graph-internal motivational origin.
- **Session** does not appear in the motivational table because Sessions
  aren't "derived from" upstream artifacts in the motivational sense; they
  reference their bundle via `prov:used` (mechanical). The Session is the
  Activity that connects mechanical provenance to motivational provenance,
  not an instance of either flavor itself.

---

## Feature feature:boundary-artifact-class

title: Define the BoundaryArtifact class for system-boundary artifacts

@motivated_by brief:dual-provenance-discipline
@addresses_decision adr:boundary-artifacts-as-orphan-class

A BoundaryArtifact is one whose motivational origin is legitimately
external to the orchestration graph. Two cases:

1. **Sensing-action outputs.** An artifact produced by a sensing action
   (monitoring read, log query, API poll, user interview, market signal,
   etc.) — see foundations doc on sensing. The motivational origin is
   external reality; the mechanical provenance traces back through the
   sensing-action's Session.
2. **Initial-request artifacts.** An artifact entering the system from
   outside (a customer request, an upstream system's Feedback, a CI-posted
   WorkerImageSubmission, a Brief authored before the system existed). The
   motivational origin is the external party; there is no graph-internal
   antecedent.

SHACL handles this by declaring BoundaryArtifact as a class; instances are
exempt from the motivational-provenance sh:or requirement. Concretely the
artifact-type shape becomes:

```turtle
:FeatureShape a sh:NodeShape ;
  sh:targetClass :Feature ;
  sh:and ( :MechanicalProvenanceShape ) ;
  sh:or (
    [ a sh:NodeShape ; sh:class :BoundaryArtifact ]
    [ sh:property [ sh:path :addresses ; sh:minCount 1 ; sh:class :Feedback ] ]
    [ sh:property [ sh:path :decomposesFrom ; sh:minCount 1 ; sh:class :Brief ] ]
    [ sh:property [ sh:path :originatedFrom ; sh:minCount 1 ; sh:class :DiscoveryFinding ] ]
    [ sh:property [ sh:path :respondsTo ; sh:minCount 1 ; sh:class :Question ] ]
  ) .
```

The first branch of the sh:or is "this is a BoundaryArtifact, exempt from
motivational requirement." Subsequent branches enumerate the motivational
predicate options for the type.

A BoundaryArtifact's metadata must still declare its external origin (the
external party, the sensing action that produced it, the CI run that posted
it, etc.) via the `external_origin` predicate. This isn't motivational
provenance in the graph sense — it doesn't traverse to another graph node —
but it preserves auditability of how the artifact entered the system.

```turtle
:BoundaryArtifactShape a sh:NodeShape ;
  sh:targetClass :BoundaryArtifact ;
  sh:property [
    sh:path :external_origin ;
    sh:minCount 1 ; sh:maxCount 1 ;
    sh:datatype xsd:string
  ] .
```

---

## Feature feature:provenance-shacl-shapes

title: Ship the SHACL shape files implementing the discipline

@motivated_by brief:dual-provenance-discipline
@addresses_decision adr:shacl-as-enforcement-mechanism

Concrete deliverables:

- `shapes/mechanical-provenance.ttl` — the universal mechanical block plus
  the Session-specific extension.
- `shapes/boundary-artifact.ttl` — the BoundaryArtifact class and shape.
- `shapes/motivational-predicates.ttl` — the predicate declarations with
  rdfs:subPropertyOf prov:wasDerivedFrom annotations and range constraints.
- Per-artifact-type shape files updated to import the above and add their
  motivational sh:or block:
  - `shapes/feature.ttl`
  - `shapes/adr.ttl`
  - `shapes/tc.ttl`
  - `shapes/dependency.ttl`
  - `shapes/brief.ttl`
  - `shapes/acknowledgement.ttl`
  - `shapes/feedback.ttl`
  - `shapes/worker-image.ttl`
  - (etc. per the table in feature:motivational-predicate-vocabulary)

Shape files live in pipeline-cli's `schemas/` directory and are consumed
by:
- pipeline-cli's GraphWriter for write-time enforcement
  (feature:graphwriter-shacl-enforcement)
- The SDK's codegen pipeline for typed Bundle and Artifact surfaces
  (per brief:pipeline-worker-slice-1's feature:shape-codegen)
- product-cli's existing audit infrastructure
- Any future system implementation

One source of truth per shape; everything else consumes from there.

---

## Feature feature:graphwriter-shacl-enforcement

title: Enforce both provenance blocks at the GraphWriter chokepoint

@motivated_by brief:dual-provenance-discipline
@addresses_decision adr:shacl-as-enforcement-mechanism

Every mutation through GraphWriter (pipeline-cli's single mutation
chokepoint per impl doc §7) runs SHACL validation against the incoming
triples before commit. Validation has three failure modes:

1. **Missing mechanical provenance.** GraphWriter is the source of these
   triples, so a missing mechanical block is an internal bug — assertion
   failure, hard error. The caller of GraphWriter (the harness's session-
   completion handler) is responsible for providing the session record;
   GraphWriter assembles the PROV-O triples from it.

2. **Missing motivational provenance AND not a BoundaryArtifact.** The
   write is rejected. GraphWriter returns a structured violation: the
   artifact ID, the type, the set of motivational predicates the type
   accepts, and the fact that none were present. The violation is itself
   a feedback artifact routed back to the producing session (or to the
   submitter, for boundary-artifact-related rejections).

3. **Type-specific shape violations** (required fields missing, edges
   pointing at wrong-typed targets, etc.). Same rejection pattern; the
   violation report names the failed property paths.

The validator is pyshacl (Python; for SDK-side defensive checks) and
oxigraph-shacl (Rust; for harness-side authoritative checks). Same shape
files, same SHACL spec, same results — drift between sides is a build-time
error per the shared-shape principle from the SDK Brief.

Validation runs on the incoming triple set composed with the current
named-graph snapshot, so cross-artifact constraints (the target of an
`addresses → Feedback` edge must exist and be of class Feedback) are
checked against the live graph.

---

## Feature feature:existing-artifact-migration

title: Migrate product-cli's existing artifacts to dual-provenance conformance

@motivated_by brief:dual-provenance-discipline
@addresses_decision adr:migration-grandfather-with-backfill

Product-cli already has Feature, ADR, TC, and Dependency artifacts in
production graphs. The discipline being introduced wasn't in effect when
they were authored; their existing provenance is informal.

Migration strategy:

1. **Audit pass.** Run a SPARQL query that classifies every existing
   artifact:
   - **Conformant**: already has both blocks. No action needed.
   - **Backfillable mechanical**: has motivational origin (via existing
     ad-hoc edges that map to the new vocabulary) but no PROV-O mechanical
     block. Migration script generates synthetic Session and Agent
     artifacts of class `:HistoricalSession` and `:HistoricalAgent`,
     attaches them with `:migrationNote` annotating the backfill
     provenance.
   - **Orphan**: has neither block, and no mappable existing edges.
     Flagged for human repair via a Feedback artifact of class
     `migration-orphan-needs-repair`.

2. **Grandfather rule.** Conformant + backfillable artifacts pass; orphans
   are quarantined (visible but flagged) until human repair lands. Reads
   continue; writes that touch orphan artifacts emit a warning.

3. **Cutover.** Once orphan count is below an agreed threshold (or zero),
   GraphWriter's SHACL enforcement is turned on for all writes. New
   writes must conform; existing reads work either way.

Migration tooling ships in slice 1: the audit query, the backfill script,
the orphan-Feedback emission. Slice 2 deals with whatever orphans remain
after the initial pass.

---

## Feature feature:full-chain-query-template

title: Define the canonical full-chain SPARQL traversal as a named query template

@motivated_by brief:dual-provenance-discipline
@addresses_decision adr:full-chain-as-query-template

The full-chain query is the canonical traversal that walks any artifact
backward to terminal origins (sensing-action outputs, initial-request
artifacts) and forward to terminal value actions. It's the query that
makes the audit principle ("did this role have the context it needed")
operationally tractable.

Shape (simplified, slice-1 form):

```sparql
PREFIX prov: <http://www.w3.org/ns/prov#>
PREFIX : <https://ddd.hafeok.com/ns#>

# Walk backward from focal artifact :X to terminal origins.
SELECT ?ancestor ?ancestor_type ?path_length WHERE {
  {
    :X (prov:wasGeneratedBy/prov:used)* ?ancestor .
  } UNION {
    :X (prov:wasDerivedFrom)* ?ancestor .   # motivational; subPropertyOf above
  }
  ?ancestor a ?ancestor_type .
  FILTER (
    ?ancestor_type IN (:BoundaryArtifact, :SensingActionOutput, :InitialRequest)
    || NOT EXISTS { ?ancestor prov:wasDerivedFrom ?_ }
  )
}
ORDER BY ?path_length
```

The query template is itself a first-class artifact in the orchestration
catalog, type `:QueryTemplate`, with versioning and provenance. Consumers
(audit role, meta-loop aggregation, human inspection tooling) reference
it by ID rather than re-deriving the SPARQL.

Forward version (walk to terminal value actions) is symmetric, traversing
the inverse predicates.

Slice 1 ships the template itself. Tooling that consumes it (audit
reports, meta-loop fitness functions, visualization) is downstream work.

---

## Acknowledgement ack:bootstrap-self-reference

@motivated_by brief:dual-provenance-discipline

This Brief defines the discipline that, once in effect, governs how
Briefs are authored. The discipline therefore predates its own enforcement
on itself: this Brief is authored as a free-form markdown working
document, not as a Brief artifact in product-cli's graph, because the
Brief artifact type doesn't exist yet (per brief:pipeline-worker-slice-1's
feature:brief-artifact-type) and the dual-provenance shapes that would
validate it don't exist yet (per this Brief's feature:provenance-shacl-shapes).

Once both ship, this Brief gets re-authored as a Brief artifact, with its
motivational origin declared as boundary-artifact class membership and
its mechanical provenance backfilled via the migration tooling.

The self-reference isn't a bug; it's the bootstrap. Slice 1 of any
recursive system has to exist outside the system temporarily.

---

## Acknowledgement ack:vocabulary-will-grow

@motivated_by brief:dual-provenance-discipline

The motivational-predicate vocabulary in feature:motivational-predicate-
vocabulary is the slice-1 set, chosen to cover existing and immediately-
proposed artifact types. It's certain to grow:

- New artifact types (proposed examples: OpenQuestion, OperationalFinding
  as a Feedback subtype, etc.) introduce new predicates.
- Existing types acquire new motivational predicate options as patterns
  emerge.
- Some current predicates may turn out to be poorly named or scoped and
  get renamed (with migration).

Vocabulary growth is a meta-decision tracked under
brief:dual-provenance-discipline's policy owner role. Each addition
requires an ADR explaining what gap it fills and what range constraints
apply. Excluded from slice 1: the versioning mechanism for the vocabulary
itself (feature:predicate-vocabulary-versioning, slice 2+).

---

## Acknowledgement ack:mechanical-provenance-requires-chokepoint

@motivated_by brief:dual-provenance-discipline

Auto-attached mechanical provenance only works because GraphWriter is the
single mutation path into the master graph (impl doc §7). If mutations
ever bypass GraphWriter — direct Oxigraph writes, side-channel triple
emission, anything — the mechanical block is no longer guaranteed and
the discipline degrades.

The architectural commitment to GraphWriter as chokepoint is therefore
load-bearing for this Brief. Slice 1 adds a fitness function that scans
for triples in the master graph without complete mechanical provenance
and flags them as evidence of chokepoint bypass. The fitness function is
itself a slice 2 deliverable per excluded feature:continuous-orphan-fitness-
function; slice 1 just establishes the invariant.

---

## Acknowledgement ack:this-brief-is-a-boundary-artifact

@motivated_by brief:dual-provenance-discipline
@references ack:bootstrap-self-reference

When this Brief is re-authored as a Brief artifact (post-bootstrap), it
will satisfy the motivational-provenance requirement via boundary-
artifact class membership: this Brief originated in design conversation
external to the orchestration graph, recorded in chat transcript form. Its
`external_origin` field will reference the transcript identifier or chat
session record.

The same pattern applies to brief:pipeline-worker-slice-1 and
brief:worker-distribution-slice-1. All three are initial-request boundary
artifacts. Future Briefs may have graph-internal motivational origin
(responding to an OperationalFinding, decomposing from a StrategicIntent
artifact) and will use the normal motivational predicates.

---

## ADR adr:two-flavors-of-provenance

@decides_for feature:mechanical-provenance-block
@decides_for feature:motivational-predicate-vocabulary

PROV-O supports a wide vocabulary of provenance relations. The framework's
needs split into two clean flavors:

- **Mechanical**: the operational record of how an artifact was physically
  produced. Universal, auto-attached, identical structure across artifact
  types. Maps to `prov:wasGeneratedBy`, `prov:wasAttributedTo`,
  `prov:generatedAtTime`, plus Session-side `prov:used` and
  `prov:wasInformedBy`.
- **Motivational**: the semantic record of why an artifact exists. Per-
  type controlled vocabulary, declared by the author, expressed as
  domain-specific predicates that are formally subtypes of
  `prov:wasDerivedFrom`.

Decision: encode both flavors explicitly, require both on every artifact
(modulo BoundaryArtifact exemption for motivational). Factor the
mechanical block out as a reusable SHACL shape; let each artifact type
declare its motivational alternatives via sh:or.

Alternatives considered:

- **One flavor only (mechanical).** Loses the ability to query "why does
  this exist" structurally. Audit principle degrades to "what did this
  session see," missing the upstream framing.
- **One flavor only (motivational).** Loses the operational lineage —
  who/which model/when. Mechanical provenance is what makes audit and
  measurement work; can't be omitted.
- **Single combined flavor with mixed predicates.** Tried briefly in
  draft; collapses cleanly only for trivial cases. Real provenance has
  asymmetric semantics across the two flavors (universal vs per-type,
  auto vs declared, required vs alternative-of-set) that argue for
  factoring them apart at the shape level.

The two-flavor split is the cleanest factoring of the real distinction.

---

## ADR adr:motivational-predicates-as-prov-derived-from-subtypes

@decides_for feature:motivational-predicate-vocabulary

Every motivational predicate (`addresses`, `decomposes_from`,
`originated_from`, `decides_for`, `validates`, `responds_to`, `audits`,
etc.) is declared as `rdfs:subPropertyOf prov:wasDerivedFrom` in the
shape files.

Consequence: the full-chain query can ignore the specific predicate names
and walk `prov:wasDerivedFrom` (or `prov:wasDerivedFrom*` for transitive)
to get all motivational ancestors uniformly. Predicate-specific queries
(`?x :addresses ?feedback`) still work for type-specific traversal; the
generic walk works for uniform traversal.

Without this, every consumer of the provenance graph would need to know
the full predicate vocabulary to walk chains. With this, generic
traversal is one predicate; specific queries are still specific.

Trade: the subPropertyOf reasoning has to be either materialized at write
time (cheap but increases triple count) or applied at query time (requires
the SPARQL engine to do property-path expansion). Slice 1 uses query-time
expansion via Oxigraph's path-traversal operators; revisit if it becomes
a hot path.

---

## ADR adr:boundary-artifacts-as-orphan-class

@decides_for feature:boundary-artifact-class

The discipline requires every artifact to have at least one motivational-
provenance edge. But the system has artifacts that legitimately have no
graph-internal motivational ancestor:

- Sensing-action outputs are derived from external reality, not from
  another graph node.
- Initial-request artifacts enter from outside the orchestration graph
  (customer requests, CI submissions, Briefs authored before the system
  existed).

Decision: introduce an explicit `BoundaryArtifact` class. SHACL shapes for
artifact types accept BoundaryArtifact class membership as a satisfying
alternative for the motivational requirement, via the first branch of
each type's sh:or.

Alternatives considered:

- **Don't allow orphans; require synthetic motivational edges.** Forces
  authors to invent fake ancestors. Discipline degrades from "every
  artifact has real origin" to "every artifact has at least a placeholder."
  Rejected.
- **Allow type-specific orphan handling without a class.** Each artifact
  type defines its own "no motivational required" condition. Works, but
  scatters the boundary concept across many shapes. Rejected for
  uniformity.
- **Class membership as adopted.** Centralizes the boundary concept;
  uniform across types. Accepted.

BoundaryArtifacts still carry the mechanical block AND an explicit
`external_origin` field documenting how they entered. The discipline holds
at the boundary; it just acknowledges that the boundary exists.

---

## ADR adr:shacl-as-enforcement-mechanism

@decides_for feature:provenance-shacl-shapes
@decides_for feature:graphwriter-shacl-enforcement

Enforcement options:

- **SHACL at write time.** Shape validation runs as part of every write
  through GraphWriter. Non-conformant writes are rejected. Producer is
  informed immediately. Strong invariant: the graph never contains
  non-conformant artifacts.
- **Runtime checks in code.** Each artifact-producing code path validates
  before writing. Distributed enforcement; easy to forget.
- **Periodic audit.** Scan the graph periodically for non-conformance,
  emit feedback. Eventually consistent; doesn't prevent bad writes, just
  surfaces them.

Decision: SHACL at write time. The graph maintains the invariant
"every artifact conforms to its shape" continuously, not eventually.
This is what makes the discipline operational rather than aspirational.

Periodic audit is layered on top as a defense-in-depth fitness function
(slice 2+), but its job is detecting chokepoint bypass rather than
catching write-time conformance failures.

Choice of SHACL specifically (vs SHEX, custom validators, JSON Schema):
SHACL is the W3C standard for RDF validation, has mature implementations
in both Python (pyshacl) and Rust (oxigraph-shacl), composes
declaratively, supports the property-path expressions the discipline
needs. No serious alternative for this substrate.

---

## ADR adr:migration-grandfather-with-backfill

@decides_for feature:existing-artifact-migration

Three options for existing artifacts already in the graph when the
discipline turns on:

- **Reject all non-conformant.** Strict but breaks production data.
- **Grandfather everything.** No discipline applied retroactively.
- **Grandfather with backfill.** Conformant artifacts pass; artifacts
  with informal provenance that maps to the new vocabulary get backfilled
  with synthetic mechanical-provenance triples and explicit migration
  annotation; orphans flagged for human repair.

Decision: option 3. Preserves production continuity while applying the
discipline as fully as the existing data allows. The synthetic Session
artifacts (`:HistoricalSession`) and Agent artifacts
(`:HistoricalAgent`) used for backfill are themselves marked as
migration artifacts and queryable as such — provenance integrity is
preserved at the meta-level even where it can't be reconstructed at the
artifact level.

Trade: backfill creates synthetic triples that don't represent real
historical events. Mitigation: synthetic triples are tagged with a
`:isMigrationBackfill true` annotation; queries and audits that need
"real provenance only" can filter them out. The annotation is part of
the discipline, not a hack.

---

## ADR adr:full-chain-as-query-template

@decides_for feature:full-chain-query-template

The full-chain traversal is needed by multiple consumers — audit role,
meta-loop aggregation, fitness functions, human inspection tooling.

Options:

- **Each consumer writes its own SPARQL.** Inconsistency, duplicate
  maintenance.
- **Hardcoded query in pipeline-cli.** Centralized but tied to code; not
  itself an artifact.
- **First-class QueryTemplate artifact.** Versioned, has provenance,
  evolves through the framework's normal discipline. Consumers reference
  it by ID.

Decision: option 3. The full-chain query becomes a QueryTemplate
artifact in the orchestration catalog with all the usual framework
properties — versioning, provenance, change-via-decision. When the
discipline grows to include new artifact types or predicate categories,
the QueryTemplate is updated as a normal decision, not a code change.

QueryTemplate as an artifact type also benefits other reusable queries
(bundle assembly queries from the SDK Brief's feature:shape-codegen,
audit queries, meta-loop aggregations). This Brief introduces the type;
other Briefs consume it.

---

## Open questions

1. **Predicate URI namespace.** Used `:addresses`, `:decomposes_from`, etc.
   with the framework's base namespace. Concrete URI scheme
   (`https://ddd.hafeok.com/ns#addresses`?) is a detail for shape
   authoring.

2. **Reasoning depth in SHACL validation.** When validating an edge like
   `:addresses → Feedback`, does the validator transitively resolve
   subClass relationships on the range (`Feedback` and its subclasses)?
   Default SHACL behavior is shallow; rdfs:subClassOf reasoning needs
   explicit enabling. Resolve during implementation.

3. **Cross-system motivational references.** Can a Feature in product-cli
   declare `addresses → Feedback` where the Feedback lives in
   pipeline-cli's orchestration graph (delivered via the artifact bus)?
   Federated PROV-O is excluded from this slice but the answer affects
   how cross-system references are encoded — opaque URIs that resolve
   later vs. resolved-eagerly references vs. inter-system schema
   declaration. Lean: opaque URIs, resolved at audit time.

4. **Boundary-artifact subclassing.** Should `SensingActionOutput`,
   `InitialRequest`, and `MigrationBackfill` all be subclasses of
   `BoundaryArtifact`, or peer classes with shared shape composition?
   Subclassing gives uniform handling; peers give type-specific shape
   constraints. Lean: subclasses with shape extensions per subtype.

5. **Migration timeline.** When does the cutover happen — once orphan
   count is "below threshold" or "zero"? The exact criterion needs to be
   set, probably by the policy owner role once the audit pass has
   produced data on orphan distribution.

6. **What about artifacts produced before any Session existed?** The very
   first artifacts written into the orchestration graph (the catalog
   bootstrap, the initial shape files, the first WorkerCurator role
   declaration) don't have producing Sessions because Sessions don't
   exist yet. Resolution: a synthetic `:BootstrapSession` is part of the
   migration tooling, attributed to a `:BootstrapAgent` representing the
   human operator who initialized the system. Acceptable because it's a
   one-time concern; subsequent writes have real Sessions.

7. **Provenance for the dual-provenance discipline itself.** This Brief is
   a boundary artifact (per ack:this-brief-is-a-boundary-artifact). What
   about the SHACL shape files that implement it? They're code artifacts
   produced by a Session (the slice-1 implementation session). Their
   mechanical provenance is the Session that authored them; their
   motivational provenance is this Brief. The discipline applies to its
   own implementation — recursively, as it should.

8. **LiteLLM-mediated LLM calls and provenance.** The worker-distribution
   Brief pulls LiteLLM into slice 1 as the LLM-call substrate. The discipline
   here is unchanged — every LLM call is still attributed to a Session via
   the worker's session record, and LiteLLM is an implementation detail of
   how the call is dispatched. But LiteLLM's logging callback POSTs call
   telemetry to a pipeline-cli reconciliation endpoint, and that telemetry
   includes cost figures LiteLLM is authoritative for. Open: should those
   incoming telemetry triples be treated as boundary-artifact contributions
   (LiteLLM is the external source) merged into the existing session record,
   or as a separate `LLMCallTelemetry` artifact type with its own mechanical
   provenance (the LiteLLM service as Agent) linked to the Session that
   triggered the call? Lean: separate artifact type; cleaner provenance
   chain when LiteLLM falls back across providers mid-call. Resolve when
   implementing.
