---
id: ADR-074
title: QualityVerdict as a sibling type to VerificationVerdict
status: accepted
features:
- FT-126
- FT-127
- FT-128
- FT-129
- FT-130
- FT-131
- FT-132
- FT-133
supersedes: []
superseded-by: []
domains:
- data-model
scope: domain
content-hash: sha256:07e371d9d96cda83a8016b5512dbf242a45cc32be7e639a119823191a32985dc
---

**Status:** Proposed

## Context

[ADR-018](ADR-018) defines `dec:VerificationVerdict` with a tight three-verdict
vocabulary (`approved` / `rejected` / `amendment-required`) and a SHACL shape
that load-bears the dispatch lifecycle: a `DispatchGroup` cannot transition to
`complete` without an `approved` verdict reachable via the paired
interpretation ([ADR-017](ADR-017), [FT-021](FT-021)). The slice-2 verifier
worker ([FT-023](FT-023)) emits these verdicts; downstream consumers
(gap_check, drift_check, feature-completion queries, fitness functions under
[ADR-014](ADR-014)) read them with the semantic "this artifact has been
verified against the feature_spec."

[ADR-073](ADR-073) introduces four new authoring pairs whose interpretation
sessions produce a judgment of *quality* — "this TC is fit for an implementer
to consume," "this graph demonstrates the TC minimally," "this spec serves the
request," "this ADR closes the preflight gap." A quality judgment is
structurally a verdict (it answers the same three questions: satisfied?,
rationale?, what next?). The shape demands the same three-verdict vocabulary
and the same SHACL constraints (rationale ≥ 20 chars, `rejected`/
`amendment-required` must cite ≥ 1 violated reference, etc.).

But it is **not** a verification verdict. A quality verdict says "this
authored artifact is fit for downstream use"; a verification verdict says
"this code satisfies these TCs." Conflating them pollutes every read site
that reads a verdict as "code verified." gap_check, feature-completion
queries, fitness functions, the slice-3 graph executor — all would have to
disambiguate "is this verdict the kind I care about?" The brief (§2.7) calls
this out as the failure to avoid.

## Decision

**Introduce a new graph artifact class `dec:QualityVerdict`, a sibling type
to `dec:VerificationVerdict`. Same three-verdict vocabulary, same SHACL
constraint shape, distinct `sh:targetClass`, and a single shape polymorphic
across all four judged artifact kinds.**

### Class

```turtle
@prefix dec:  <https://decision-cli.dev/ns#> .
@prefix sh:   <http://www.w3.org/ns/shacl#> .
@prefix prov: <http://www.w3.org/ns/prov#> .
@prefix xsd:  <http://www.w3.org/2001/XMLSchema#> .

dec:QualityVerdict a sh:NodeShape ;
    sh:targetClass dec:QualityVerdict ;
    sh:property [
        sh:path dec:verdict ;
        sh:in ( "approved" "rejected" "amendment-required" ) ;
        sh:minCount 1 ; sh:maxCount 1 ;
    ] ;
    sh:property [
        sh:path dec:rationale ;
        sh:datatype xsd:string ;
        sh:minCount 1 ; sh:maxCount 1 ;
        sh:minLength 20 ;
    ] ;
    sh:property [
        sh:path prov:wasGeneratedBy ;        # the judge's interpretation session
        sh:minCount 1 ; sh:maxCount 1 ;
    ] ;
    sh:property [
        sh:path prov:used ;                  # the authored artifact + bundle
        sh:minCount 1 ;
    ] ;
    sh:property [
        sh:path dec:inStream ;
        sh:minCount 1 ; sh:maxCount 1 ;
    ] ;
    sh:property [
        sh:path dec:judges ;                 # NEW: the authored artifact under judgment
        sh:minCount 1 ; sh:maxCount 1 ;
    ] ;
    sh:property [
        sh:path dec:against ;                # NEW: source-of-truth the artifact is judged against
        sh:minCount 1 ;
    ] ;
    # Conditional: rejected/amendment-required MUST cite ≥ 1 violated reference.
    sh:sparql [
        sh:message "rejected and amendment-required quality verdicts must cite at least one violated reference via dec:violates" ;
        sh:select """
            PREFIX dec: <https://decision-cli.dev/ns#>
            SELECT $this WHERE {
              $this dec:verdict ?v .
              FILTER(?v IN ("rejected", "amendment-required"))
              FILTER NOT EXISTS { $this dec:violates ?ref }
            }
        """ ;
    ] ;
    sh:property [
        sh:path dec:amendmentGuidance ;
        sh:datatype xsd:string ;
        sh:maxCount 1 ;
    ] ;
    sh:sparql [
        sh:message "amendment-required quality verdicts must carry dec:amendmentGuidance" ;
        sh:select """
            PREFIX dec: <https://decision-cli.dev/ns#>
            SELECT $this WHERE {
              $this dec:verdict "amendment-required" .
              FILTER NOT EXISTS { $this dec:amendmentGuidance ?g }
            }
        """ ;
    ] .
```

The shape is **load-bearing identical** to [ADR-018](ADR-018)'s
`VerificationVerdictShape` for the five fields that constrain the three
verdicts (`dec:verdict`, `dec:rationale`, `prov:wasGeneratedBy`, `prov:used`,
`dec:inStream`), plus the conditional `dec:violates` and `dec:amendmentGuidance`
shape. The two **new predicates** are what make the verdict polymorphic across
artifact kinds:

- **`dec:judges` (1)** — the authored artifact under judgment. IRI of a
  `dec:TestCriterion`, `dec:VerificationGraph`, `dec:FeatureSpec`, or `dec:ADR`
  in the product graph (or a per-dispatch proposal artifact pending
  acceptance).
- **`dec:against` (≥ 1)** — the source-of-truth the artifact is judged for
  fitness against. For a TC quality verdict: the feature_spec the TC must
  serve. For a VG quality verdict: the TC(s) it claims to cover plus the
  environment. For a spec quality verdict: the originating request/brief. For
  an ADR quality verdict: the preflight gap (the unacknowledged cross-cutting
  ADR or domain) plus the feature_spec.

### One shape, all four kinds (polymorphism)

The brief (§2.10) decides: **one `dec:QualityVerdict` shape across all four
judged kinds**, polymorphic via `dec:judges` + `dec:against`. Not four per-kind
shapes. The polymorphism mirrors `dec:verifies` over TC/Feature in TC-057 —
the same predicate carries different referent classes depending on the
artifact type, and SHACL constrains the verdict's structure uniformly.

| Judged artifact kind | `dec:judges` referent | `dec:against` referent(s) |
|---|---|---|
| TC | `dec:TestCriterion` (or proposed-TC artifact) | `dec:FeatureSpec` |
| VerificationGraph | `dec:VerificationGraph` (or `GraphProposal`) | one or more `dec:TestCriterion` + one `dec:VerificationEnvironment` |
| feature_spec | `dec:FeatureSpec` (or `SpecProposal`) | the originating request artifact |
| ADR | `dec:ADR` (or `AdrProposal` or `Acknowledgement`) | the preflight gap + the feature_spec |

A per-kind SHACL fragment refines `dec:against`'s expected referent class via
a conditional `sh:sparql` shape per kind, but the QualityVerdict node shape
itself is single and reused across all four.

### Why a sibling type and not `VerificationVerdict` reuse

The brief (§2.7) settles this. A verdict read anywhere in the graph must
unambiguously mean "code verified against feature_spec," because that is
what downstream consumers ([ADR-014](ADR-014) fitness functions,
[ADR-022](ADR-022)–[ADR-026](ADR-026) feedback routing, [ADR-021](ADR-021)
agreement metric, feature-completion queries) wired in around it. Quality
verdicts answer a different question for different consumers
([ADR-076](ADR-076)'s planner reads them; the [ADR-075](ADR-075) fitness
function on auto-accept watches them). Two classes is the right axis to split
them on.

Three concrete failure modes the split avoids:

1. **Polluting `product verify`.** `product verify FT-XXX` walks every TC
   linked to a feature and runs its runner. If quality verdicts were
   `VerificationVerdict` instances, a query asking "what's the verification
   verdict for this TC?" would return both code-passing-the-TC verdicts and
   TC-is-fit-to-consume verdicts. The semantics of "verified" collapses.
2. **Polluting the agreement metric.** [ADR-021](ADR-021) measures
   action-interpretation agreement on verification verdicts. Mixing in
   quality verdicts (a different rubric on a different artifact kind) makes
   the metric meaningless.
3. **Polluting `dec:DispatchGroup` lifecycle.** [FT-021](FT-021)'s SHACL
   refuses `complete` without an `approved` VerificationVerdict for code
   dispatches. A QualityVerdict on an authoring dispatch needs the same
   SHACL gate but on its own class — separating the classes lets each
   dispatch type carry its own pairing assertion without aliasing.

### Why same vocabulary and same shape

The brief (§2.10) also fixes this: not a different verdict vocabulary, not a
different cardinality on rationale. The two classes share the verdict
*language* because the three answers (`approved` / `rejected` /
`amendment-required`) are general — they describe how an interpretation
session relates to an action, regardless of what the action produced.
Diverging the vocabulary would force every consumer to relearn a new
controlled list per artifact kind; aligning lets the planner's read predicate
([ADR-073](ADR-073) §"What the planner observes") use the same constants.

The amendment-loop machinery ([FT-021](FT-021) `awaiting-amendment` → next
action dispatch consuming `dec:amendmentGuidance`) carries over unchanged for
authoring pairs — an `amendment-required` quality verdict re-dispatches the
author with the guidance, same shape as the verifier's amendment loop.

### Where the class lives

The shape is added to `crates/decision-cli/src/core/ontology/quality.ttl`
(new file, parallel to `verdict.ttl`). The `core` location follows the
slice-level SDP discipline ([ADR-016](ADR-016)): all features in the
readiness chain depend on the shape, none depend on others. Codegen for the
Pydantic model on the worker SDK side
([FT-080](FT-080)/[FT-085](FT-085) pattern) emits a `QualityVerdict` mirror
when the shape lands.

## Rejected alternatives

- **Reuse `dec:VerificationVerdict` with a discriminator field
  (e.g. `dec:verdictKind: code | tc | vg | spec | adr`).** Rejected per §2.7:
  every existing read site that consumes "verified" semantics would need a
  filter clause, and forgetting the filter creates silent semantic drift.
  The class boundary is the right axis to split on; a discriminator inside a
  shared class is a worse failure mode (no compile-time/SHACL safety against
  forgetting the filter).
- **Four per-kind shapes (`TcQualityVerdict`, `VgQualityVerdict`,
  `SpecQualityVerdict`, `AdrQualityVerdict`).** Rejected per §2.10: forks the
  artifact model. The judgment shape is identical across kinds; the only
  variance is the referent class of `dec:against`, which is captured by a
  per-kind conditional SHACL fragment, not a per-kind shape.
- **Free-form `dec:judgmentType` literal instead of class-based
  polymorphism.** Same family as the discriminator above, with the added
  failure mode of unbounded prose drift. Rejected.
- **Different verdict vocabulary (e.g. `fit-for-purpose` / `not-fit` /
  `revise`).** Rejected: forces every consumer to learn a new language per
  artifact kind. The three-verdict vocabulary in [ADR-018](ADR-018) is
  general; quality verdicts inherit it.
- **No SHACL — let workers emit whatever JSON they want.** Rejected for the
  same reason [ADR-018](ADR-018) rejected free-form verdicts: invites prose
  drift, prevents downstream aggregation, breaks the amendment loop's
  guidance-handling shape.

## Consequences

**Positive:**

- Read sites stay clean: any query asking "what's the verification verdict
  for this code?" gets `VerificationVerdict` only; any query asking "what's
  the quality verdict for this TC/VG/spec/ADR?" gets `QualityVerdict` only.
- Authoring pair lifecycle ([ADR-073](ADR-073)) gets a dedicated artifact
  that downstream readers (planner per [ADR-076](ADR-076), acceptance per
  [ADR-075](ADR-075), fitness functions per [ADR-014](ADR-014)) can target
  unambiguously.
- Existing slice-2 verifier code path is unchanged. Polymorphism over the
  four authored kinds is captured in one new shape, not four.
- Worker SDK codegen ([FT-085](FT-085)) emits one new Pydantic model
  (`QualityVerdict`); the four judge workers share it.

**Negative / accepted costs:**

- One new ontology file and one new class to maintain. Schema migrations
  affecting both verdict classes will require parallel work; the alignment
  in shape is deliberate so the migration is mechanical.
- Codegen and SPARQL queries that previously hardcoded
  `dec:VerificationVerdict` need updating to read both classes where the
  question is "what's the latest verdict of any kind?" The split is the
  point — those queries are rare and the explicit choice is correct.

**Enforcement:**

- SHACL shape in `core::ontology::quality::shape` validates QualityVerdict
  structure at write time. The same StreamWriter chokepoint
  ([ADR-005](ADR-005)) that gates VerificationVerdict writes gates these
  writes.
- A TC in the readiness-orchestrator cluster asserts every authored
  DispatchGroup in `complete` status has a paired QualityVerdict with
  `dec:verdict = approved`.
- A TC asserts the polymorphism: a tc-quality verdict's `dec:judges`
  resolves to a `dec:TestCriterion`, a vg-quality verdict's resolves to a
  `dec:VerificationGraph`, etc. The per-kind conditional SHACL fragment is
  the carrier.

## Status

Proposed. Linked to [ADR-073](ADR-073) (the role lifecycle that produces
these verdicts), [ADR-075](ADR-075) (which decides how an `approved`
QualityVerdict transitions to "observed by the planner"), and
[ADR-076](ADR-076) (which reads QualityVerdicts to flip readiness bits). The
SHACL fragment ships under a feature spec authored alongside the four
authoring features (see [FT-126](FT-126)–[FT-131](FT-131)).
