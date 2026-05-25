---
id: ADR-049
title: pyoxigraph as the in-memory bundle store on the worker
status: proposed
features:
- FT-078
supersedes: []
superseded-by: []
domains: []
scope: feature-specific
---

## Context

The bundle is an in-memory RDF sub-graph for the duration of one worker
session (see ADR-050 — the Session IS a `prov:Activity`, and the bundle is
its working memory). Two candidate in-memory triple-store implementations
for Python:

- **pyoxigraph:** Python binding to Oxigraph, the same Rust engine the
  harness uses. Same SPARQL evaluator, same SHACL implementation, same
  serialization tooling.
- **rdflib:** pure-Python, mature, broad ecosystem, but a different SPARQL
  implementation with different performance characteristics and subtle
  behavioral differences.

## Decision

pyoxigraph for the worker's in-memory bundle store.

## Consequences

- **Positive:** Zero semantic drift between sides of the wire. A bundle
  query written for harness-side assembly can be unit-tested in-process in
  the worker SDK against pyoxigraph and behave identically. SHACL
  validation results are the same engine on both sides — defensive
  validation in FT-080 means the same thing as authoritative validation in
  GraphWriter.
- **Positive:** Performance. Oxigraph is fast; for slice-1 bundle sizes
  (hundreds to low-thousands of triples) the bundle store is effectively
  free.
- **Positive:** N-Quads on the wire (ADR-046) parses natively into
  pyoxigraph — no intermediate format.
- **Negative:** Native dependency. pyoxigraph ships pre-built wheels for
  major platforms, but workers on exotic architectures may need to build
  from source. Acceptable for slice 1 (Linux/macOS x86_64 + arm64 covers
  the deployment target).

## Alternatives considered

- **rdflib.** Rejected: different SPARQL engine, different SHACL
  implementation, opens room for "passes locally, fails at GraphWriter"
  bugs that are exactly what we're trying to eliminate.
- **No in-memory store; query the harness over the wire.** Rejected:
  defeats the bundle as a closed, deterministic input to the session.
  Worker code would become network-bound for every accessor call, and
  replay (ADR-future, slice 2) would have to mock all those calls.

## References

- `feature:session-layer` (FT-078) owns the per-session pyoxigraph store.
- ADR-050 (Session as PROV-O Activity) frames the store as Activity working
  memory.
- ADR-046 (N-Quads wire format) — pyoxigraph consumes N-Quads natively.