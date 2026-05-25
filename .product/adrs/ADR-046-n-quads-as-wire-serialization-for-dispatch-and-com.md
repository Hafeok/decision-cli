---
id: ADR-046
title: N-Quads as wire serialization for dispatch and completion payloads
status: proposed
features:
- FT-077
supersedes: []
superseded-by: []
domains: []
scope: domain
---

## Context

Dispatch and completion payloads carry RDF, not arbitrary JSON — both sides
treat the graph as the source of truth, and the wire is just the transport
between two graph stores (the harness's Oxigraph store and the worker's
in-memory pyoxigraph store, see ADR-049).

Two serialization candidates:

- **N-Quads:** preserves named-graph membership, no information loss at the
  boundary, terse, line-oriented (streamable, diffable).
- **JSON-LD:** workers see structured objects ergonomically, but requires a
  context document agreed across sides and loses some serialization
  guarantees around blank-node scope.

## Decision

N-Quads on the wire. The SDK converts to a Python-friendly view internally
(typed accessors generated from SHACL — see ADR-048 and FT-079) so worker
code never touches raw quads anyway. Fidelity at the boundary, ergonomics
one layer above.

## Consequences

- **Positive:** No information loss across the wire. Named graphs survive
  the round trip. Blank-node scope is unambiguous.
- **Positive:** Line-oriented format streams naturally over both SSE and
  HTTP POST. Easy to inspect with standard tooling (`grep`, `wc`, RDF
  CLIs).
- **Positive:** Same serialization the harness uses internally — no extra
  JSON-LD context-mapping step on either side.
- **Negative:** Workers that bypass the SDK's typed accessors and read
  N-Quads directly need an RDF parser. Mitigated by pyoxigraph being the
  default in-memory store and accepting N-Quads natively.

## Alternatives considered

- **JSON-LD on the wire.** Rejected for slice 1: requires a stable context
  document agreed across the harness and SDK release boundaries, and the
  ergonomics win disappears once typed accessors land. Revisit if/when a
  non-Python SDK needs to be written and JSON-LD tooling there proves
  meaningfully stronger than N-Quads tooling.
- **Turtle on the wire.** Has the same fidelity for a single graph but
  doesn't carry named-graph membership; rejected because the harness sends
  multi-named-graph bundles.

## References

- `feature:wire-layer` (FT-077) implements N-Quads framing.
- ADR-049 (pyoxigraph as the worker's in-memory store) consumes N-Quads
  natively.