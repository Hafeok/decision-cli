---
id: ADR-050
title: Session as a direct materialization of the PROV-O Activity
status: accepted
features:
- FT-078
supersedes: []
superseded-by: []
domains: []
scope: domain
content-hash: sha256:3b9240eb4b3164a8333e37a1454ef368cf5613354e31eb90c9d745f9bc04e3e9
---

## Context

The worker SDK introduces a `Session` abstraction: one dispatch ⇒ one
session, owns the bundle store, accumulates telemetry, posts completion.
Two ways to relate this SDK abstraction to the PROV-O activity model the
harness already uses (ADR-004 — PROV-O for events and sessions):

- **Adjacent:** Session is a Python wrapper that creates a `prov:Activity`
  in the harness's record at some point during its lifetime.
- **Identity:** Session IS the `prov:Activity` — the in-process Session
  object is the live form, the harness's session record is the persistent
  form, and they share the same identity (URI).

## Decision

Identity. The SDK's `Session.id` is the URI of the `prov:Activity` in the
harness's PROV-O graph. The in-process Session is the live materialization;
the harness's session record is the durable form; they are the same
conceptual entity.

Consequence: mechanical provenance annotations on artifacts the session
produces (`prov:wasGeneratedBy`, `prov:wasAttributedTo`, `prov:used`) trace
through the Session URI as the central node. The worker does NOT duplicate
these annotations on the wire — the harness's GraphWriter populates them
from the session record at write time, deriving from the (session, artifact,
bundle) tuple it already has.

## Consequences

- **Positive:** One concept, one identity, one provenance graph traversal.
  "What activity generated this artifact?" answers with a single URI
  whether you ask the live SDK or the harness's stored graph.
- **Positive:** Eliminates a class of bugs where session-side and PROV-O-side
  identifiers diverge or where mechanical provenance is emitted twice with
  slight differences.
- **Positive:** Aligns with the dual-provenance discipline (ADR-038, ADR-041,
  and FT-069 / FT-073) — mechanical provenance is the GraphWriter's job at
  the chokepoint, not the worker's job in emitted triples. The Session-as-
  Activity identity is what makes that boundary clean.
- **Negative:** SDK `Session` lifetime must be carefully tied to harness's
  Activity lifetime (start, end timestamps must agree). Mitigated by the
  harness controlling activity start (dispatch event) and end (completion
  acceptance).

## Alternatives considered

- **Adjacent (Session creates an Activity).** Rejected: two identities to
  reconcile, two sources of truth for session metadata. Encourages bugs
  where workers attach mechanical provenance themselves and the harness
  attaches different mechanical provenance.

## References

- `feature:session-layer` (FT-078) materializes the Session and depends on
  FT-069 / FT-073 for the mechanical-provenance machinery.
- ADR-004 (PROV-O for events and sessions) on the harness side.
- ADR-038 (dual provenance: mechanical + motivational) — the framework
  this decision plugs into.
- ADR-041 (SHACL at GraphWriter chokepoint) — the enforcement boundary
  this decision relies on.
- FT-069 (mechanical-provenance SHACL fragment), FT-073 (GraphWriter SHACL
  enforcement), FT-075 (full-chain provenance query) — the concrete
  features that make this contract verifiable.