---
id: ADR-038
title: 'Dual provenance: mechanical (PROV-O auto-attached) and motivational (per-type predicate vocabulary)'
status: accepted
features:
- FT-069
- FT-070
- FT-101
- FT-071
- FT-072
- FT-073
- FT-074
supersedes: []
superseded-by: []
domains:
- data-model
scope: platform
content-hash: sha256:423e71fcdf24805a875840cbca820e9d378c8f36352e17d879c13e3a40607733
---

## Context

The framework has committed to PROV-O as the provenance substrate (see `docs/ddd/Implementing_DDD.md` §3, and ADR-004: PROV-O for events and sessions), but has not specified the discipline that governs how every artifact type uses it. Without an explicit rule, artifacts accumulate with inconsistent provenance: some carrying full PROV-O, some carrying ad-hoc fields, some carrying nothing. The audit principle — *"did this role have the context a competent human in this role would have?"* — depends on being able to walk back through provenance to read what informed a decision. Without uniform discipline that property is best-effort and fragile.

PROV-O itself supports a wide vocabulary of provenance relations. The framework's needs split into two clean flavors that have asymmetric semantics:

| Aspect | Mechanical | Motivational |
|---|---|---|
| What it records | How the artifact was physically produced | Why the artifact exists |
| Universal? | Yes — identical structure for every artifact type | No — per-type controlled vocabulary |
| Who authors? | GraphWriter, auto-attached from session record | The producing worker / author, declared explicitly |
| PROV-O mapping | `prov:wasGeneratedBy`, `prov:wasAttributedTo`, `prov:generatedAtTime`, plus session-side `prov:used` / `prov:wasInformedBy` | Domain-specific predicates declared as `rdfs:subPropertyOf prov:wasDerivedFrom` (see ADR-039) |
| Required cardinality | One block, always | At least one of the per-type alternatives (or BoundaryArtifact membership; see ADR-040) |

## Decision

**Encode both flavors explicitly. Require both on every artifact** (modulo the BoundaryArtifact exemption from ADR-040 for the motivational side).

- **Mechanical block.** Factored out as a reusable SHACL NodeShape (`:MechanicalProvenanceShape`) that every artifact-type shape composes in via `sh:and`. Three required triples: `prov:wasGeneratedBy → Session` (1..1), `prov:wasAttributedTo → Agent` (1..n), `prov:generatedAtTime` (xsd:dateTime, 1..1). Workers do not author these — the harness's session-completion handler hands the session record to GraphWriter, and GraphWriter materializes the triples.
- **Motivational block.** Per artifact type, the shape's `sh:or` lists the set of acceptable motivational predicates (FT-070 catalogs them). At least one must be present, unless the artifact is a `BoundaryArtifact` (ADR-040).
- **Session itself** carries a separate `:SessionProvenanceShape` with `prov:used` (the bundle), `prov:wasInformedBy` (prior sessions whose outputs were in the bundle), and `prov:wasAssociatedWith` (the Agent). The Session is the Activity that connects mechanical provenance to motivational provenance; it is not itself an instance of either flavor.

Both blocks are enforced at the GraphWriter chokepoint via SHACL (ADR-041).

### Alternatives considered

- **One flavor only (mechanical).** Loses the ability to query "why does this exist" structurally. The audit principle degrades to "what did this session see," missing the upstream framing.
- **One flavor only (motivational).** Loses the operational lineage — who/which model/when. Mechanical provenance is what makes audit and measurement work; cannot be omitted.
- **Single combined flavor with mixed predicates.** Tried briefly in draft; collapses cleanly only for trivial cases. Real provenance has asymmetric semantics across the two flavors (universal vs per-type, auto vs declared, required vs alternative-of-set) that argue for factoring them apart at the shape level.

The two-flavor split is the cleanest factoring of the real distinction.

## Consequences

**Positive.**

- The audit principle is operationally tractable: walk `wasGeneratedBy → Session, used → Artifact` (mechanical) and `wasDerivedFrom*` (motivational) to reconstruct the full chain from any artifact back to terminal origins. See FT-075 / ADR-043 for the canonical traversal.
- Adding a new artifact type is mechanical: declare `sh:targetClass`, `sh:and` the universal mechanical block, `sh:or` the motivational alternatives. Three lines.
- Mechanical provenance is uniform across the entire system, so every consumer that needs "what did this come from" has a single shape to read.

**Negative / accepted costs.**

- Every artifact carries an extra ~3 triples of mechanical provenance plus 1..n motivational triples. Storage cost is negligible; query cost is one extra join.
- Authors of new artifact types must explicitly enumerate motivational predicates for the type (FT-070) rather than handing the validator a single inherited rule.

**Boundary enforcement.** The discipline only holds because GraphWriter is the single mutation chokepoint (`docs/ddd/Implementing_DDD.md` §7 and decision-cli's own FT-001). Side-channel writes bypass the discipline. ADR-041 plus the slice-2 orphan fitness function (excluded from slice 1 per the Brief) defend the chokepoint.

## Relationship to existing ADRs

- **ADR-004 (PROV-O for events and sessions).** Extends — ADR-004 chose PROV-O as the substrate; this ADR specifies how every artifact type uses it.
- **ADR-002 (Graph-as-state).** Compatible — provenance triples are graph state like any other.
- **ADR-016 (Vertical-slice with compile-time SDP).** Compatible — the mechanical SHACL fragment lives in `core/`; per-type motivational vocabulary is extended by slices.

## Status

Proposed. Foundational for FT-069 (mechanical block), FT-070 (motivational vocabulary), and by transitivity FT-072 / FT-073 / FT-074 / FT-075. Authored as part of the decomposition of `brief:dual-provenance-discipline`, which is itself a boundary-artifact Brief originating in design conversation external to the orchestration graph.
