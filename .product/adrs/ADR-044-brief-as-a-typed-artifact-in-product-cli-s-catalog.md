---
id: ADR-044
title: Brief as a typed artifact in product-cli's catalog
status: accepted
features:
- FT-076
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
content-hash: sha256:923d511bfc6351f7078f085815a03fffcf96612ffe3f3722fe46e968dc1104b5
---

## Context

Bounds documents have historically been free-form markdown wrappers around the
structured product-cli graph (see `decision-cli-slice-1-bounds.md` and
`docs/ddd/Implementing_DDD.md` §11). This works only because no slice's bounds
have ever needed to be referenced by other artifacts: the markdown sits next
to the spec, the human reader picks it up out-of-band, no edges cross from
typed artifacts back into the markdown.

That assumption breaks the moment one Brief needs to reference another (a
later slice's Brief checking the prior Brief's `excludes` list to make sure
nothing was silently re-promised; a Feature asking "what scope decision
motivated me?"). Without a typed shape, those references resolve only by
prose convention, with no validation, no queryability, and no audit trail.

The pipeline-worker-slice-1 working session is the first concrete instance
where this matters: it explicitly excludes other Features (`feature:replay-
driver-impl`, `feature:custom-provider-adapters`, etc.) that need to be
queryable to prevent re-promising.

## Decision

Introduce `Brief` as a typed artifact in product-cli's catalog with the shape
developed in the pipeline-worker-slice-1 working session:

- Fields: `title`, `premise`, `goal`, `success_criteria` (markdown bodies, no
  fixed sub-structure).
- Edges:
  - `decomposes_into → Feature[]` — Features this Brief authorizes.
  - `excludes → Feature[]` — explicit non-goals tracked for re-promising
    detection.
  - `acknowledges → Acknowledgement[]` — recorded debts the Brief takes on.
  - `references → Artifact[]` — out-of-band references (docs, sibling Briefs).

Future bounds documents are authored as Brief artifacts, not free-form
markdown.

## Consequences

- **Positive:** Bounds become queryable. `product brief excludes BRIEF-X`
  surfaces what was explicitly out-of-scope; a future Feature attempting to
  silently re-promise something excluded by an earlier Brief becomes a
  detectable graph anomaly rather than a process failure.
- **Positive:** Briefs participate in PROV-O. Features can attribute their
  motivation to a Brief, and the full provenance chain becomes traversable.
- **Negative:** Schema-migration cost. Until FT-076 ships, working-session
  Briefs remain as markdown wrappers; the first Brief authored this way will
  need to be back-migrated.
- **Negative:** One more artifact type to maintain. Mitigated by the type
  being structurally trivial (a wrapper with edges).

## Alternatives considered

- **Extend `Feature` with a parent edge for scope grouping.** Rejected:
  conflates "what gets shipped" with "what frames the shipping." A Feature
  has a status, a phase, and a `complete` lifecycle; a Brief never reaches
  `complete` — it's done when its decomposed Features ship.
- **Keep bounds documents free-form, link by URL.** Rejected: no schema
  means no validation, no queryability, no discipline. The whole point of
  product-cli is to make engineering artifacts first-class graph residents,
  and bounds documents are engineering artifacts.

## References

- `brief:pipeline-worker-slice-1` (working session that surfaced this
  decision).
- `docs/ddd/Implementing_DDD.md` §11 (slice-1 strategy and bounds-document
  convention).